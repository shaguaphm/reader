use crate::{
  utils::reader
};
use crate::{log_if_err, wrap_err};
use anyhow::{Result};
use crate::utils::reader_config::*;

type CmdResult<T = ()> = Result<T, String>;

/// get all profiles from `profiles.yaml`
#[tauri::command]
pub fn print_log(message: String) {
  println!("{}", message);
}

#[tauri::command]
pub fn get_config() -> CmdResult<ReaderConfig> {
  let reader_config = ReaderConfig::new();
  Ok(reader_config)
}

#[tauri::command]
pub fn save_config(config: Option<ReaderConfig>) -> CmdResult<bool> {
  let mut reader_config = ReaderConfig::new();
  if let Some(config) = config {
    log_if_err!(reader_config.patch_config(config));
  }
  Ok(true)
}

#[tauri::command]
pub fn check_java(java_path: String) -> CmdResult<String> {
  if java_path.is_empty() {
    return match reader::check_installed_java() {
      Ok(java_path) => Ok(java_path),
      Err(err) => Err(err.to_string())
    }
  } else {
    return match reader::check_java_version(java_path.clone()) {
      Ok(()) => Ok(java_path),
      Err(err) => Err(err.to_string())
    }
  }
}

#[tauri::command]
pub fn get_server_port() -> CmdResult<u64> {
  let reader_config = ReaderConfig::new();
  Ok(reader::get_server_port(&reader_config))
}

#[tauri::command]
pub fn is_server_running() -> CmdResult<bool> {
  Ok(reader::is_server_running())
}

#[tauri::command]
pub fn start_server(
  app_handle: tauri::AppHandle
) -> CmdResult {
  wrap_err!(reader::start_server(&app_handle))
}

#[tauri::command]
pub fn stop_server() -> CmdResult {
  wrap_err!(reader::stop_server())
}

#[tauri::command]
pub fn restart_server(
  app_handle: tauri::AppHandle
) -> CmdResult {
  if let Ok(_) = reader::stop_server() {
    wrap_err!(reader::start_server(&app_handle))
  } else {
    Err(format!("未知错误"))
  }
}