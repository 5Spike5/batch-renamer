/// 重命名线程 → 主线程的消息
#[derive(Debug, Clone)]
pub enum RenameEvent {
    /// 进度更新：当前处理到第几个、总共几个、正在处理哪个文件
    Progress {
        current: usize,
        total: usize,
        current_file:String
    },
    /// 全部完成：成功数、失败数
    Finished {
        success: usize,
        failed: usize,
    },
    /// 发生致命错误
    Error(String),
}