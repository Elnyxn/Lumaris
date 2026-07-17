// 正式版本 Windows GUI 子系统，无控制台
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    lumaris_lib::run();
}
