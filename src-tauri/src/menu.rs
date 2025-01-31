use crate::memos;
use crate::runtime_config::RuntimeConfig;
use dialog::{confirm_dialog, info_dialog, MessageType};

use crate::fl;
use crate::localize::fl;
use log::{debug, error, warn};
use std::convert::AsRef;
use strum_macros::AsRefStr;
use strum_macros::FromRepr;
#[cfg(target_os = "macos")]
use tauri::menu::AboutMetadata;
use tauri::{
    async_runtime,
    menu::{Menu, MenuEvent, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    AppHandle, Manager, Runtime,
};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;
use tokio::time::{self, Duration, Instant};
use url::Url;

#[derive(AsRefStr, FromRepr, Clone, Copy)]
enum MainMenu {
    #[strum(serialize = "app")]
    App,
    #[strum(serialize = "settings")]
    AppSettings,
    #[strum(serialize = "browse-data-directory")]
    AppBrowseDataDirectory,
    #[strum(serialize = "check-for-updates")]
    GlobalUpdate,
    #[strum(serialize = "view")]
    View,
    #[strum(serialize = "developer-tools")]
    ViewDevTools,
    #[strum(serialize = "hide-menu-bar")]
    ViewHideMenuBar,
    #[strum(serialize = "refresh")]
    ViewRefresh,
    #[strum(serialize = "reload-view")]
    ViewReload,
    #[strum(serialize = "window")]
    Window,
    #[strum(serialize = "help")]
    Help,
    #[strum(serialize = "memospot-version")]
    HelpMemospotVersion,
    #[strum(serialize = "documentation")]
    HelpMemospotDocumentation,
    #[strum(serialize = "release-notes")]
    HelpMemospotReleaseNotes,
    #[strum(serialize = "report-issue")]
    HelpMemospotReportIssue,
    #[strum(serialize = "memos-version")]
    HelpMemosVersion,
    #[strum(serialize = "documentation")]
    HelpMemosDocumentation,
    #[strum(serialize = "release-notes")]
    HelpMemosReleaseNotes,
}
impl MainMenu {
    pub fn index(&self) -> usize {
        *self as usize
    }
}

/// Update menu after Memos version is known.
///
/// Display current Memos version in the help menu.
pub fn update_with_memos_version<R: Runtime>(handle: &AppHandle<R>) {
    const INTERVAL_MS: u64 = 100;
    const TIMEOUT_MS: u128 = 15000;

    let handle_ = handle.clone();
    async_runtime::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(INTERVAL_MS));
        let time_start = Instant::now();

        loop {
            interval.tick().await;
            if time_start.elapsed().as_millis() > TIMEOUT_MS {
                debug!(
                    "Unable to set Memos version in menu. Timed out ({}ms).",
                    TIMEOUT_MS
                );
                break;
            }
            if !memos::get_version().is_empty() {
                break;
            }
        }

        let Some(main_window) = handle_.get_webview_window("main") else {
            error!("Unable to set Memos version in menu. Main window not found.");
            return;
        };

        // Find and update the Memos version in the Help menu.
        if let Some(menu) = main_window.menu() {
            let version_text = format!("Memos v{}", memos::get_version());

            menu.items()
                .iter()
                .flat_map(|item| item.iter())
                .filter_map(|menu| menu.as_submenu())
                .find_map(|submenu| {
                    submenu
                        .get(&MainMenu::HelpMemosVersion.index().to_string())
                        .and_then(|entry| entry.as_menuitem().cloned())
                })
                .map(|menuitem| menuitem.set_text(version_text));
        }
    });
}

pub fn build_empty<R: Runtime>(handle: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    Menu::with_items(handle, &[])
}

pub fn build<R: Runtime>(handle: &AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    let config = RuntimeConfig::from_global_store();
    if config.yaml.memospot.window.hide_menu_bar == Some(true) {
        return build_empty(handle);
    }

    let check_for_updates = MenuItemBuilder::with_id(
        MainMenu::GlobalUpdate.index(),
        fl(MainMenu::GlobalUpdate.as_ref()),
    )
    .build(handle)?;

    #[cfg(target_os = "macos")]
    let app_name = handle.config().product_name.clone().unwrap_or_default();

    #[cfg(target_os = "macos")]
    let mac_menu = &SubmenuBuilder::new(handle, app_name)
        .about(Some(AboutMetadata::default()))
        .separator()
        .text(
            MainMenu::AppSettings.index(),
            fl(MainMenu::AppSettings.as_ref()),
        )
        .item(&check_for_updates)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let app_menu = &SubmenuBuilder::new(handle, fl(MainMenu::App.as_ref()))
        .items(&[
            &MenuItemBuilder::with_id(
                MainMenu::AppSettings.index(),
                fl(MainMenu::AppSettings.as_ref()),
            )
            .accelerator("CmdOrCtrl+S")
            .build(handle)?,
            &MenuItemBuilder::with_id(
                MainMenu::AppBrowseDataDirectory.index(),
                fl(MainMenu::AppBrowseDataDirectory.as_ref()),
            )
            .accelerator("CmdOrCtrl+D")
            .build(handle)?,
            &PredefinedMenuItem::separator(handle)?,
            #[cfg(target_os = "macos")]
            &PredefinedMenuItem::close_window(handle, None)?,
            #[cfg(not(target_os = "macos"))]
            &PredefinedMenuItem::quit(handle, None)?,
            #[cfg(not(target_os = "macos"))]
            &check_for_updates,
        ])
        .build()?;

    let view_menu = &SubmenuBuilder::new(handle, fl(MainMenu::View.as_ref()))
        .items(&[
            #[cfg(target_os = "macos")]
            &PredefinedMenuItem::fullscreen(handle, None)?,
            #[cfg(any(debug_assertions, feature = "devtools"))]
            &MenuItemBuilder::with_id(
                MainMenu::ViewDevTools.index(),
                fl(MainMenu::ViewDevTools.as_ref()),
            )
            .accelerator("CmdOrCtrl+Shift+I")
            .build(handle)?,
            &MenuItemBuilder::with_id(
                MainMenu::ViewHideMenuBar.index(),
                fl(MainMenu::ViewHideMenuBar.as_ref()),
            )
            .accelerator("CmdOrCtrl+H")
            .build(handle)?,
            &MenuItemBuilder::with_id(
                MainMenu::ViewRefresh.index(),
                fl(MainMenu::ViewRefresh.as_ref()),
            )
            .accelerator("F5")
            .build(handle)?,
            &MenuItemBuilder::with_id(
                MainMenu::ViewReload.index(),
                fl(MainMenu::ViewReload.as_ref()),
            )
            .accelerator("CmdOrCtrl+R")
            .build(handle)?,
        ])
        .build()?;

    #[cfg(target_os = "macos")]
    let window_menu = &SubmenuBuilder::new(handle, fl(MainMenu::Window.as_ref()))
        .items(&[
            &PredefinedMenuItem::minimize(handle, None)?,
            &PredefinedMenuItem::maximize(handle, None)?,
            &PredefinedMenuItem::separator(handle)?,
            &PredefinedMenuItem::close_window(handle, None)?,
        ])
        .build()?;

    let help_menu = &SubmenuBuilder::new(handle, fl(MainMenu::Help.as_ref()))
        .item(
            &MenuItemBuilder::with_id(
                MainMenu::HelpMemospotVersion.index(),
                format!("Memospot v{}", handle.package_info().version),
            )
            .enabled(false)
            .build(handle)?,
        )
        .separator()
        .text(
            MainMenu::HelpMemospotDocumentation.index(),
            fl(MainMenu::HelpMemospotDocumentation.as_ref()),
        )
        .text(
            MainMenu::HelpMemospotReleaseNotes.index(),
            fl(MainMenu::HelpMemospotReleaseNotes.as_ref()),
        )
        .text(
            MainMenu::HelpMemospotReportIssue.index(),
            fl(MainMenu::HelpMemospotReportIssue.as_ref()),
        )
        .item(
            &MenuItemBuilder::with_id(
                MainMenu::HelpMemosVersion.index(),
                format!("Memos v{}", memos::get_version()),
            )
            .enabled(false)
            .build(handle)?,
        )
        .separator()
        .text(
            MainMenu::HelpMemosDocumentation.index(),
            fl(MainMenu::HelpMemosDocumentation.as_ref()),
        )
        .text(
            MainMenu::HelpMemosReleaseNotes.index(),
            fl(MainMenu::HelpMemosReleaseNotes.as_ref()),
        )
        .build()?;

    Menu::with_items(
        handle,
        &[
            #[cfg(target_os = "macos")]
            mac_menu,
            app_menu,
            view_menu,
            #[cfg(target_os = "macos")]
            window_menu,
            help_menu,
        ],
    )
}

pub fn handle_event<R: Runtime>(handle: &AppHandle<R>, event: MenuEvent) {
    let mut webview = handle.get_webview_window("main").unwrap();
    let open_link = |url| {
        handle.opener().open_url(url, None::<&str>).ok();
    };

    #[cfg(debug_assertions)]
    debug!("menu event: {:?}", event);

    let Ok(event_id) = event.id().0.parse::<usize>() else {
        return;
    };

    match MainMenu::from_repr(event_id).unwrap() {
        MainMenu::AppBrowseDataDirectory => {
            let config = RuntimeConfig::from_global_store();
            handle
                .opener()
                .open_url(
                    config.paths.memospot_data.to_string_lossy().to_string(),
                    None::<&str>,
                )
                .ok();
        }
        MainMenu::AppSettings => {
            let handle_ = handle.clone();
            tauri::async_runtime::spawn(async move {
                let window_builder = tauri::WebviewWindowBuilder::new(
                    &handle_,
                    "settings",
                    tauri::WebviewUrl::App("/settings".into()),
                )
                .title(fl(MainMenu::AppSettings.as_ref()).replace("&", ""))
                .center()
                .min_inner_size(800.0, 600.0)
                .inner_size(1160.0, 720.0)
                .disable_drag_drop_handler()
                .visible(false)
                .menu(build_empty(&handle_).unwrap());

                #[cfg(not(target_os = "macos"))]
                window_builder.build().ok();
                #[cfg(target_os = "macos")]
                window_builder
                    .title_bar_style(tauri::TitleBarStyle::Overlay)
                    .build()
                    .ok();
            });
        }
        MainMenu::GlobalUpdate => {
            let handle_ = handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(update) = handle_.updater().unwrap().check().await.unwrap() {
                    let user_confirmed = confirm_dialog(
                        fl!("dialog-update-title").as_str(),
                        fl!("dialog-update-message", version = update.version).as_str(),
                        MessageType::Info,
                    );
                    if user_confirmed {
                        handle_
                            .opener()
                            .open_url(update.download_url.as_str(), None::<&str>)
                            .ok();
                    } else {
                        warn!("User declined update download.");
                    }
                } else {
                    info_dialog(fl!("dialog-update-no-update").as_str());
                }
            });
        }
        #[cfg(any(debug_assertions, feature = "devtools"))]
        MainMenu::ViewDevTools => {
            webview.open_devtools();
        }
        MainMenu::ViewHideMenuBar => {
            if let Some(main_window) = handle.get_webview_window("main") {
                main_window.remove_menu().ok();
            }
        }
        MainMenu::ViewRefresh => {
            let url = webview.url().unwrap().join("./").unwrap();
            webview.navigate(url).ok();
        }
        MainMenu::ViewReload => {
            handle.set_menu(build(handle).unwrap()).ok();
            let url = Url::parse(if cfg!(debug_assertions) {
                "http://localhost:1420" // Same as build.devUrl in Tauri.toml.
            } else {
                "tauri://localhost"
            })
            .unwrap();
            webview.navigate(url).ok();
        }
        MainMenu::HelpMemospotDocumentation => {
            open_link("https://memospot.github.io/");
        }
        MainMenu::HelpMemospotReleaseNotes => {
            let url = format!(
                "https://github.com/memospot/memospot/releases/tag/v{}",
                handle.package_info().version
            );
            open_link(url.as_str());
        }
        MainMenu::HelpMemospotReportIssue => {
            open_link("https://github.com/memospot/memospot/issues/new");
        }
        MainMenu::HelpMemosDocumentation => {
            open_link("https://usememos.com/docs");
        }
        MainMenu::HelpMemosReleaseNotes => {
            let url = format!(
                "https://www.usememos.com/changelog/{}",
                memos::get_version().replace(".", "-")
            );
            open_link(url.as_str());
        }
        _ => {
            error!("unhandled menu event: {}", event.id().0)
        }
    }
}
