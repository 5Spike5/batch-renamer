use std::sync::mpsc::Sender;

use slint::ComponentHandle;

use crate::{App, model::rename_event::RenameEvent};

pub fn bind_events(app :&App,tx :Sender<RenameEvent>) {
    let app_weak = app.as_weak();
    let tx_clone = tx.clone();
    //用户选择文件夹
    app.on_choose_folder_clicked(move ||{
        todo!("1. rfd::FileDialog 选文件夹
              2. rename_service::scan_directory() 扫描
              3. 构造 VecModel<FileEntry> → app.set_files()
              4. app.set_folder_path()")
    });

    //用户点击预览（传入当前规则模板，Rust 侧重新计算 new-name）
    app.on_preview_clicked(|rule|{
        println!("规则: {}", rule)
    });

    //用户点击“执行重命名”
    app.on_execute_clicked(||{
        todo!()
    });
    
    //用户勾选/取消某一行（传入 index + 新状态）
    app.on_item_checked_changed(|idx,is_checked|{
        println!("第{idx}个文件，选中状态{is_checked}");
        todo!()
    });
}