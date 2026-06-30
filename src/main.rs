slint::include_modules!();
mod model;
mod controller;
mod service;
use std::{sync::{Arc, Mutex}, time::Duration};
use slint::{ModelRc, Timer, VecModel};
use crate::{model::rename_event::RenameEvent, service::rename_service::scan_directory};
fn main() ->Result<(),slint::PlatformError>{
    // 1. 创建 Slint 窗口
    let app = App::new()?;
    
    //2.创建通道
    let (tx,rx) = std::sync::mpsc::channel::<RenameEvent>();
    let rx = Arc::new(Mutex::new(rx));

    //3.绑定 callback —— 传 &App，不要 move 所有权
    controller::app_controller::bind_events(&app, tx.clone());

    //4.启动 Timer（轮询 rx，更新 UI）
    let timer = {
        let app_weak = app.as_weak();
        let rx_clone = rx.clone();
        let timer = Timer::default();
        timer.start(slint::TimerMode::Repeated, Duration::from_millis(100), move ||{
            let rx_guard = rx_clone.lock().unwrap();
            loop {
                use std::sync::mpsc::TryRecvError;
                match rx_guard.try_recv() {
                    Ok(RenameEvent::Progress { current, total, .. } ) =>{
                        if let Some(app) = app_weak.upgrade() {
                            app.set_progress_current(current as i32);
                            app.set_progress_total(total as i32);
                            app.set_status_text("正在处理...".into());
                        }
                    },
                    Ok(RenameEvent::Finished { success, failed }) => {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_is_running(false);
                            app.set_status_text(
                                if failed > 0 {"部分完成(有失败)".into()}
                                else  { "全部完成 ✓".into() }
                            );
                            // TODO: 刷新文件列表（重新 scan_directory + set_files）
                            //1. 获取当前文件夹路径
                            let path = app.get_folder_path().to_string();
                            if !path.is_empty() {
                                //2.重新扫描目录
                                if let Ok(files) = service::rename_service::scan_directory(&path) {
                                    //3.重建FileEntry列表
                                    let entries:Vec<FileEntry> = files.iter().enumerate().map(|(i,name)|{
                                        FileEntry{
                                            old_name:name.into(),
                                            new_name:name.into(),
                                            checked:true,
                                            index:i as i32
                                        }
                                    }).collect();
                                    app.set_files(ModelRc::new(VecModel::from(entries)));
                                }
                            }
                            //4.设置结果摘要
                            app.set_renamer_result(RenameResult{
                                total:(success + failed) as i32,
                                success:success as i32,
                                failed:failed as i32
                            });
                        }
                    },
                    Ok(RenameEvent::Error(msg)) => {
                        if let Some(app) = app_weak.upgrade() {
                            app.set_status_text(format!("错误:{}",msg).into());
                        }
                    },
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break
                }
            }
        });
        timer
    };

    app.run()
}
