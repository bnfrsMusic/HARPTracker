// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// use harp_tracker_lib::track_lib::tracker::Tracker;



pub mod track_lib;


fn main() {
    harp_tracker_lib::run();
}
