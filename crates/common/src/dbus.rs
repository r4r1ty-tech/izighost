pub const DBUS_SERVICE_NAME: &str = "com.izighost.Daemon";
pub const DBUS_OBJECT_PATH: &str = "/com/izighost/Daemon";
pub const DBUS_INTERFACE_NAME: &str = "com.izighost.Daemon";

// В интерфейсе com.izighost.Daemon определены следующие методы:
// - StartRvms() -> u32 (pipewire_node_id)
// - StopRvms() -> ()
// - SendChatMessage(text: String) -> ()
// - TriggerOcr() -> ()
// - StartListening() -> ()
// - StopListening() -> ()
// - ListProfiles() -> Vec<String>
// - GetProfile(id: String) -> Profile
// - SaveProfile(profile: Profile) -> Profile
// - DeleteProfile(id: String) -> ()
// - SetActiveProfile(id: String) -> ()
// - GetActiveProfile() -> Option<Profile>
//
// И сигналы:
// - ChatChunk(delta_text: String)
// - ChatCompleted()
// - OcrCompleted(text: String)
// - AsrCompleted(text: String)
// - ErrorOccurred(message: String)
