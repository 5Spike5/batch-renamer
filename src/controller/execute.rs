use std::sync::mpsc::Sender;

use crate::{App, model::rename_event::RenameEvent};



pub fn bind_execute(app:&App,tx:Sender<RenameEvent>) {
    
}