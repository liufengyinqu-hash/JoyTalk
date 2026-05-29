fn main() {
    const COMMANDS: &[&str] = &[
        "joycon_get_status",
        "joycon_get_mappings",
        "joycon_set_mapping",
        "joycon_reset_mappings",
        "joycon_set_enabled",
        "joycon_get_enabled",
        "joycon_list_actions",
        "joycon_start_capture",
        "joycon_stop_capture",
        "joycon_list_presets",
        "joycon_get_preset_mappings",
        "joycon_load_preset",
        "joycon_load_preset_from_url",
        "joycon_export_mappings",
        "joycon_import_mappings",
        "joycon_list_apps",
        "joycon_get_imu",
        "joycon_set_imu",
        "joycon_get_profiles",
        "joycon_save_profile",
        "joycon_delete_profile",
        "joycon_set_per_app_enabled",
        "joycon_get_per_app_enabled",
        "joycon_get_frontmost",
        "joycon_get_frontmost_app",
    ];

    tauri_plugin::Builder::new(COMMANDS).build();
}
