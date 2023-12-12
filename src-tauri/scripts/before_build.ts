/*
 * This script runs before the tauri build command.
 * pnpm ts-node .\src-tauri\scripts\before_build.ts
 */
import { findRepoRoot } from "./common.ts";
import { Readable } from "node:stream";
import { finished } from "node:stream/promises";
import * as fs from "node:fs";
import * as crypto from "node:crypto";
import decompress from "decompress";
// import { GitHubRelease, GitHubAsset } from "./types.d.ts";
import type { GitHubRelease, GitHubAsset } from "./types.d.ts";
function makeTripletFromFileName(file: string): string {
    const os = (() => {
        if (file.includes("darwin")) {
            return "darwin";
        } else if (file.includes("linux")) {
            return "linux";
        } else if (file.includes("windows")) {
            return "windows";
        }
    })();

    const platform = (() => {
        if (file.includes("windows")) {
            return "pc";
        } else if (file.includes("linux")) {
            return "unknown";
        } else if (file.includes("darwin")) {
            return "apple";
        }
    })();

    const arch = (() => {
        if (file.includes("arm64") && file.includes("darwin")) {
            return "aarch64";
        }
        if (file.includes("arm64") || file.includes("aarch64")) {
            return "arm64";
        } else {
            return "x86_64";
        }
    })();

    const compiler = (() => {
        if (file.includes("windows")) {
            return "msvc";
        } else if (file.includes("linux")) {
            return "gnu";
        }
    })();

    const triplet = [arch, platform, os, compiler].join("-");
    if (triplet.endsWith("-")) {
        return triplet.slice(0, -1);
    }
    return triplet;
}

async function downloadServerBinaries() {
    const repo = "lincolnthalles/memos-builds";
    const repoUrl = `https://api.github.com/repos/${repo}/releases/latest`;
    const downloadFilesMask = [
        "memos-*-darwin-arm64.tar.gz",
        "memos-*-darwin-x86_64.tar.gz",
        "memos-*-linux-x86_64.tar.gz",
        "memos-*-windows-x86_64.zip",
    ];

    // fetch data from github api
    const response = await fetch(repoUrl);
    const json = (await response.json()) as GitHubRelease;
    const assets = json.assets as GitHubAsset[];

    if (!assets || assets.length == 0) {
        throw new Error("Failed to fetch assets");
    }
    const tag = json.tag_name;

    const sha256sums = assets.find((asset) => {
        return asset.name.endsWith("SHA256SUMS.txt");
    })?.browser_download_url;

    if (!sha256sums) {
        throw new Error("Failed to find SHA256SUMS.txt");
    }

    console.log(`Latest ${repo} tag: ${tag}`);

    let selectedFiles: GitHubAsset[] = [];
    for (const asset of assets) {
        // glob-like matching
        for (const mask of downloadFilesMask) {
            const regex = new RegExp(mask.replace("*", ".*"));
            if (asset.name.match(regex)) {
                selectedFiles.push(asset);
            }
        }
    }

    if (selectedFiles.length == 0) {
        throw new Error("Failed to match files");
    } else {
        console.log(`Matched ${selectedFiles.length} files`);
    }

    // download files
    for (const file of selectedFiles) {
        const downloadUrl = file.browser_download_url as string;
        const fileName = file.name;
        const filePath = `./server-dist/${fileName}`;
        const fileExists = (await fs).existsSync(filePath);
        if (!fileExists) {
            console.log(`Downloading ${fileName}...`);
            const res = await fetch(downloadUrl);
            const fileStream = fs.createWriteStream(filePath, { flags: "wx" });
            if (!res.body) {
                throw new Error("No response body");
            }
            await finished(Readable.fromWeb(res.body as any).pipe(fileStream));
        } else {
            console.log(`File ${fileName} already exists and will be reused.`);
        }
    }

    // check hashes via memos_SHA256SUMS.txt
    const sha256response = await fetch(sha256sums);
    const data = await sha256response.text();
    const lines = data.split("\n");
    const fileHashes: Record<string, string> = {};
    for (const line of lines) {
        if (line.length == 0) {
            continue;
        }
        const elements = line.split("  ");
        const hash = elements[0].trim();
        const fileName = elements[1].trim();
        fileHashes[fileName] = hash;
    }

    // console.log(fileHashes);

    for (const file of selectedFiles) {
        const fileName = file.name;
        console.log(`Checking hash for ${fileName}...`);

        const filePath = `./server-dist/${fileName}`;
        // resolve path
        const fileBuffer = (await fs).readFileSync(filePath);
        const fileHash = crypto
            .createHash("sha256")
            .update(fileBuffer)
            .digest("hex");
        console.log(`Hash: ${fileHash}`);
        if (fileHash !== fileHashes[fileName]) {
            throw new Error(`Hash mismatch for ${fileName}`);
        }
    }

    // extract files
    const extractDir = "./server-dist/extracted";
    const extractDirExists = fs.existsSync(extractDir);
    if (!extractDirExists) {
        fs.mkdirSync(extractDir);
    }

    for (const file of selectedFiles) {
        const fileName = file.name;
        const filePath = `./server-dist/${fileName}`;
        if (fileName.endsWith(".zip") || fileName.endsWith(".tar.gz")) {
            console.log(`Extracting ${fileName}...`);
            await decompress(filePath, extractDir).then((files) => {
                console.log(`Extracted ${files.length} files`);
            });
        }

        let exe = fileName.includes("windows") ? ".exe" : "";

        // rename memos binary to a target triple
        const triplet = makeTripletFromFileName(fileName);

        fs.renameSync(
            `${extractDir}/memos${exe}`,
            `./server-dist/memos-${triplet}${exe}`,
        );
        fs.rmSync(extractDir, { recursive: true });
        fs.rmSync(filePath);
    }
}

async function main() {
    const repoRoot = await findRepoRoot();
    console.log(`Repo root: ${repoRoot}`);
    process.chdir(repoRoot);
    console.log("Running `before build` hooks...");

    const serverDistDir = "./server-dist";
    const serverDistDirExists = fs.existsSync(serverDistDir);
    if (!serverDistDirExists) {
        fs.mkdirSync(serverDistDir);
    }

    await downloadServerBinaries();
}
//     const rustInfo = (await execa("rustc", ["-vV"])).stdout;
//     const targetTriple = /host: (\S+)/g.exec(rustInfo)[1];
//     if (!targetTriple) {
//         console.error("Failed to determine platform target triple");
//     }
//     fs.renameSync(
//         `src-tauri/binaries/sidecar${extension}`,
//         `src-tauri/binaries/sidecar-${targetTriple}${extension}`,
//     );
// }

main().catch((e) => {
    throw e;
});