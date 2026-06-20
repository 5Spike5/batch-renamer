use std::sync::mpsc::Sender;
use slint::ComponentHandle;

use crate::{App, controller::{check, choose_folder, execute, preview}, model::rename_event::RenameEvent};

pub fn bind_events(app :&App,tx :Sender<RenameEvent>) {
    //用户选择文件夹
    choose_folder::bind_choose_folder(app);
    //用户点击预览（传入当前规则模板，Rust 侧重新计算 new-name）
    preview::bind_preview(app);
    //用户点击“执行重命名”
    execute::bind_execute(app, tx);
    //用户勾选/取消某一行（传入 index + 新状态）
    check::bind_check(app);
}