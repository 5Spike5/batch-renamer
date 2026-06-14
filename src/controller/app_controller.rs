use std::sync::mpsc::Sender;

use crate::{App, model::rename_event::RenameEvent};

pub fn bind_events(app :App,tx :Sender<RenameEvent>) {
    //用户点击文件夹
    app.on_choose_folder_clicked(||{
        todo!()
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