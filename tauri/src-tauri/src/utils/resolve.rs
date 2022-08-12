use crate::log_if_err;
use crate::utils::init;
use crate::utils::reader;
use crate::utils::reader_config::*;
use std::sync::{Arc, Mutex};
use tauri::{App, Manager};
use tauri::utils::config::AppUrl;
use url::Url;

/// handle something when start app
pub fn resolve_setup(app: &App) {
  // setup a simple http server for singleton
  //   server::embed_server(&app.handle());

  // init app config
  init::init_app(app.package_info());

  let reader_config = ReaderConfig::new();
  log::info!("配置文件: {:?}", reader_config);

  // create main window
  create_main_window(app, &reader_config);

  // start reader server
  // start_server_with_config(app, &reader_config);
}

fn create_main_window(app: &App, config: &ReaderConfig) {
  let reader_config = Arc::new(Mutex::new(config.clone()));
  let app_handle = app.app_handle();

  let server_port = reader::get_server_port(&config);

  let debug_web = if let Some(_debug_web) = config.debug {
    if _debug_web {
      "1"
    } else {
      ""
    }
  } else {
    ""
  };

  let api = format!("http://localhost:{}/reader3", server_port);
  let default_window_url = tauri::WindowUrl::App(format!("index.html?api={}", api).into());
  #[allow(unused_assignments)]
  let mut window_url = default_window_url.clone();

  #[cfg(debug_assertions)] {
    let build_config = app.config().build.clone();
    log::info!("app.config.build {:?}", build_config);
    let index_url = build_config.dev_path;

    window_url = match index_url {
      AppUrl::Url(dev_window_url) => match dev_window_url {
        tauri::WindowUrl::App(path) => tauri::WindowUrl::App(format!("{}?api={}&debug={}", path.into_os_string().into_string().unwrap(), api, debug_web).into()),
        tauri::WindowUrl::External(url) => {
          let mut new_url = url.clone();
          new_url.query_pairs_mut()
            .append_pair("api", &api)
            .append_pair("debug", debug_web);
          tauri::WindowUrl::External(new_url)
        },
        _ => default_window_url.clone()
      },
      _ => default_window_url.clone()
    };
  }

  if let Some(window_url_config) = &config.window_url{
    if !window_url_config.is_empty() {
      if window_url_config.starts_with("http://") || window_url_config.starts_with("https://") {
        match Url::parse_with_params(window_url_config, &[("api", api), ("debug", debug_web.to_string())]) {
          Ok(url) => {
            window_url = tauri::WindowUrl::External(url);
          }
          Err(err) => {
            log::info!("config.window_url {} error {}", window_url_config, err);
          }
        }
      }
    }
  }

  log::info!("window_url {}", window_url);

  let mut builder =
    tauri::window::WindowBuilder::new(&app_handle, "main", window_url)
      .title("Reader")
      // .center()
      .fullscreen(false);

  let config = reader_config.lock().unwrap();

  if let Some(set_window_size) = config.set_window_size {
    if set_window_size {
      if config.width.is_some() && config.height.is_some() {
        let width = config.width.unwrap();
        let height = config.height.unwrap();
        log::info!("set size to ({}, {})", width, height);
        builder = builder.inner_size(width, height);
      }
    }
  }

  if let Some(set_window_position) = config.set_window_position {
    if set_window_position {
      if config.position_x.is_some() && config.position_y.is_some() {
        let position_x = config.position_x.unwrap();
        let position_y = config.position_y.unwrap();
        log::info!("set position to ({}, {})", position_x, position_y);
        builder = builder.position(position_x, position_y);
      }
    } else {
      builder = builder.center();
    }
  }

  drop(config);

  match builder
    // .center()
    .decorations(true)
    // .decorations(false)
    // .transparent(true)
    .build()
  {
    Ok(window) => {
      let scale_factor = window.scale_factor().unwrap();
      window.on_window_event(move |event| match event {
        tauri::WindowEvent::Resized(size) => {
          let logical_size = size.to_logical::<f64>(scale_factor);
          log::info!("resize {:?}", logical_size);
          let mut config = reader_config.lock().unwrap();
          if let Some(remember_size) = config.remember_size {
            if remember_size {
              config.width = Some(logical_size.width);
              config.height = Some(logical_size.height);
              log_if_err!(config.save_file());
              // log_if_err!(app.emit_all("size-changed", logical_size));
            }
          }
          drop(config);
        }
        tauri::WindowEvent::Moved(position) => {
          let logical_position = position.to_logical::<f64>(scale_factor);
          log::info!("moved {:?}", logical_position);
          let mut config = reader_config.lock().unwrap();
          if let Some(remember_position) = config.remember_position {
            if remember_position {
              config.position_x = Some(logical_position.x);
              config.position_y = Some(logical_position.y);
              log_if_err!(config.save_file());
              // log_if_err!(app.emit_all("position-changed", logical_position));
            }
          }
          drop(config);
        }
        _ => {
          log::info!("event: {:?}", event);
        }
      })
    }
    Err(err) => log::error!(target: "app", "{err}"),
  }

  log::info!("main window created");
}

// create main window
// pub fn create_window(app_handle: &AppHandle, name: &str, title: &str, url: tauri::WindowUrl) {
//   if let Some(window) = app_handle.get_window(name) {
//     let _ = window.unminimize();
//     let _ = window.show();
//     let _ = window.set_focus();
//     return;
//   }

//   log::info!("url {}", url);
//   let builder = tauri::window::WindowBuilder::new(app_handle, name, url)
//     .title(title)
//     .center()
//     .fullscreen(false);
//   // .min_inner_size(600.0, 520.0);

//   #[cfg(target_os = "windows")]
//   {
//     use crate::utils::winhelp;
//     use window_shadows::set_shadow;
//     use window_vibrancy::apply_blur;

//     match builder
//       .decorations(false)
//       .transparent(true)
//       // .inner_size(800.0, 636.0)
//       .build()
//     {
//       Ok(_) => {
//         let app_handle = app_handle.clone();

//         tauri::async_runtime::spawn(async move {
//           if let Some(window) = app_handle.get_window("main") {
//             let _ = window.show();
//             let _ = set_shadow(&window, true);

//             if !winhelp::is_win11() {
//               let _ = apply_blur(&window, None);
//             }
//           }
//         });
//       }
//       Err(err) => log::error!(target: "app", "{err}"),
//     }
//   }

//   #[cfg(target_os = "macos")]
//   crate::log_if_err!(builder
//     .decorations(true)
//     // .inner_size(800.0, 620.0)
//     .build());

//   #[cfg(target_os = "linux")]
//   crate::log_if_err!(builder
//     .decorations(false)
//     .transparent(true)
//     // .inner_size(800.0, 636.0)
//     .build());
// }
