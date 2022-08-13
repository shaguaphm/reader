use crate::log_if_err;
use crate::utils::dirs;
use crate::utils::help;
use crate::utils::reader_config::*;

use anyhow::{bail, Result};
use tauri::api::process::CommandChild;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::sleep;
use std::time::Duration;
use tauri::api::process::{Command, CommandEvent};
use tauri::AppHandle;

static READER_SERVER_RUNNING: AtomicBool = AtomicBool::new(false);
static READER_SERVER_START_FAILED: AtomicBool = AtomicBool::new(false);
static mut READER_SERVER_COMMAND_CHILD: Option<CommandChild> = None;

pub fn is_server_running() -> bool {
  READER_SERVER_RUNNING.load(Ordering::Relaxed)
}

pub fn start_server(app: &AppHandle) -> Result<()> {
  let config = ReaderConfig::new();
  start_server_with_config(app, &config)
}

pub fn start_server_with_config(app: &AppHandle, reader_config: &ReaderConfig) -> Result<()> {
  let mut java_path = String::from("");
  if let Some(java_path_config) = &reader_config.java_path {
    java_path = java_path_config.clone();
    if let Err(err) = check_java_version(java_path.clone()) {
      log::error!("check java {}", err);
      java_path = String::from("");
    }
  }
  if java_path.is_empty() {
    if let Ok(java) = check_installed_java() {
      java_path = java;
    }
  }

  if java_path.is_empty() {
    bail!(format!("请安装 Java8 以上环境!"));

  // if !java_path.is_empty() {
    // if let Some(window) = &app.get_window("main") {
    //   log::info!("打开设置页面");
    //   log_if_err!(window.eval("function gotoSettingPage(){var url = window.location.origin + window.location.pathname + window.location.search + window.location.hash.replace(/^[^?]*\\??/, '#/setting?');console.log('gotoSettingPage', url);window.location.assign(url);}window.addEventListener('DOMContentLoaded', gotoSettingPage);window.addEventListener('load', gotoSettingPage);if(window.location.search){gotoSettingPage()}"));
    // } else {
    //   log::error!("找不到主窗口...");
    //   return;
    // }
  } else {
    // 启动 jar
    if let Err(err) = launch_server(app, java_path, reader_config) {
      bail!(format!("启动 reader 接口服务失败! {}", err));
    }
    log::info!("打开主窗口");
  }
  Ok(())
}

pub fn stop_server() -> Result<()> {
  unsafe {
    if let Some(reader_command) = READER_SERVER_COMMAND_CHILD.take() {
      reader_command.kill()?;
    }
  }
  Ok(())
}

pub fn check_installed_java() -> Result<String> {
  if let Ok(java) = which::which("java") {
    log::info!("java path {}", java.display());
    let java_path = java.into_os_string().into_string().unwrap();
    return match check_java_version(java_path.clone()) {
      Ok(()) => Ok(java_path),
      Err(err) => Err(err)
    };
  }
  bail!(format!("请安装 Java8 以上环境!"));
}

pub fn check_java_version(java_path: String) -> Result<()> {
  // let output = if cfg!(target_os = "windows") {
  //   Command::new(java.into_os_string().into_string().unwrap())
  //     .args(["-version"])
  //     .output()
  //     .expect("failed to execute process")
  // } else {
  //   Command::new(java.into_os_string().into_string().unwrap())
  //     .args(["-version"])
  //     .output()
  //     .expect("failed to execute process")
  // };
  let output =
    Command::new(java_path)
      .args(["-version"])
      .output();
  if let Err(err) = output {
    bail!(format!("请检查 java 路径 {}", err))
  }
  let output = output.unwrap();
  log::info!("output {:?}", output);
  if output.status.success() {
    log::info!("stderr {}", output.stderr);
    let result = output
      .stderr
      .split('\n')
      .collect::<Vec<_>>()
      .get(0)
      .unwrap()
      .split(' ')
      .collect::<Vec<_>>();
    // log::info!("result {:?}", result);

    let result2 = result.get(2).unwrap().split('.').collect::<Vec<_>>();
    let main_ver = result2
      .get(0)
      .unwrap()
      .replace("\"", "")
      .parse::<i32>()
      .unwrap();
    let sub_ver = result2
      .get(1)
      .unwrap()
      .replace("\"", "")
      .parse::<i32>()
      .unwrap();
    log::info!("{}", format!("main_ver: {main_ver} sub_ver: {sub_ver}"));
    if main_ver == 1 && sub_ver < 8 {
      bail!(format!("java 版本不能低于 8"))
    }
  } else {
    bail!(format!("获取 java 版本号失败，请检查 java 命令!"))
  }

  return Ok(());
}

fn launch_server(app: &AppHandle, java_path: String, reader_config: &ReaderConfig) -> Result<()> {
  let jar_path = dirs::reader_jar_path()
      .display().to_string();
  log::info!("jar path {}", jar_path);

  let args = prepare_args(jar_path, reader_config);

  let (mut rx, _child) = Command::new(java_path)
    .args(args)
    .current_dir(dirs::app_home_dir())
    .spawn()
    .expect("Failed to spawn reader server");

  unsafe {
    READER_SERVER_COMMAND_CHILD = Some(_child);
  }

  tauri::async_runtime::spawn(async move {
    while let Some(event) = rx.recv().await {
      match event {
        CommandEvent::Stdout(line) | CommandEvent::Stderr(line) => {
          log::info!("[SERVER] {}", line);
          if !READER_SERVER_RUNNING.load(Ordering::Relaxed) {
            if let Some(_index) = line.find("ReaderApplication Started") {
              log::info!(
                "find Started ReaderApplication {} set result {}",
                _index,
                READER_SERVER_RUNNING.fetch_or(true, Ordering::Relaxed)
              );
            }
          }
        }
        CommandEvent::Terminated(payload) => {
          log::info!("Reader server exit with code {}", payload.code.unwrap_or_default());
          READER_SERVER_START_FAILED.fetch_or(true, Ordering::Relaxed);
          break;
        }
        // CommandEvent::Error(error) => {
        //   log::error!("Reader server error {}", error);
        // }
        _ => {}
      }
    }
  });
  wait_for_server_ready(app);
  return Ok(());
}

pub fn get_server_port(reader_config: &ReaderConfig) -> u64 {
  let mut server_port = 8080;
  if let Some(_server_port) = reader_config.server_port {
    server_port = _server_port;
  }
  return server_port;
}

fn prepare_args(jar_path: String, reader_config: &ReaderConfig) -> Vec<String> {
  let mut args = Vec::new();
  args.push(String::from("-jar"));
  args.push(jar_path);

  if let Some(server_config) = &reader_config.server_config {
    for item in server_config.iter() {
      if item.0.as_str() == Some("reader.app.workDir") {
        log::warn!("无效设置 reader.server.workDir");
        continue;
      }
      if item.0.as_str() == Some("reader.server.port") {
        log::warn!("请使用 serverPort 设置监听端口，reader.server.port 无效");
        continue;
      }

      if let Some(name) = item.0.as_str() {
        if item.1.is_bool() {
          args.push(format!("--{}={}", name, item.1.as_bool().unwrap()));
        } else if item.1.is_string() {
          let value = item.1.as_str().unwrap();
          if !value.is_empty() {
            args.push(format!("--{}={}", name, value));
          }
        } else if item.1.is_u64() {
          args.push(format!("--{}={}", name, item.1.as_u64().unwrap()));
        }
      }
    }
  }

  let server_port = get_server_port(&reader_config);

  if server_port > 0 {
    args.push(format!("--reader.server.port={}", server_port));
  }

  args.push(format!("--reader.app.workDir={}", dirs::app_home_dir().into_os_string().into_string().unwrap()));
  return args;
}

fn wait_for_server_ready(_app: &AppHandle) {
  let start_time = help::get_now();
  loop {
    // log::info!(
    //   "READER_SERVER_RUNNING {}",
    //   READER_SERVER_RUNNING.load(Ordering::Relaxed)
    // );
    if READER_SERVER_START_FAILED.load(Ordering::Relaxed) || help::get_now() > start_time + 30 {
      log::info!(
        "reader server launch failed!!"
      );
      // app.exit(1);
      log_if_err!(stop_server());
      break;
    }
    if READER_SERVER_RUNNING.load(Ordering::Relaxed) {
      break;
    }
    sleep(Duration::from_millis(300));
  }
}
