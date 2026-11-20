// AI Studio is a desktop application; debug builds also use the GUI subsystem.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    ai_studio_lib::run();
}
