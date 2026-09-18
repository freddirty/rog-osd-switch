/// ASUS vendor VCP 0xEB values used by DisplayWidget Center.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OsdCommand {
    Close = 0,
    Show = 1,
    Up = 2,
    Down = 3,
    Right = 4,
    Left = 5,
    Enter = 6,
    Back = 7,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Hotkey {
    Up,
    Down,
    Right,
    Left,
}

/// Maps every shortcut directly to a virtual monitor-joystick direction.
///
/// When the OSD is closed, the monitor firmware interprets Right as its
/// dedicated Input Select shortcut. When an OSD is already open, the same
/// directional commands navigate it. The application therefore needs no
/// menu-state tracking and must not send the separate Show command.
pub fn command_for_hotkey(hotkey: Hotkey) -> OsdCommand {
    match hotkey {
        Hotkey::Up => OsdCommand::Up,
        Hotkey::Down => OsdCommand::Down,
        Hotkey::Right => OsdCommand::Right,
        Hotkey::Left => OsdCommand::Left,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_shortcuts_to_monitor_joystick_directions() {
        assert_eq!(command_for_hotkey(Hotkey::Up), OsdCommand::Up);
        assert_eq!(command_for_hotkey(Hotkey::Down), OsdCommand::Down);
        assert_eq!(command_for_hotkey(Hotkey::Right), OsdCommand::Right);
        assert_eq!(command_for_hotkey(Hotkey::Left), OsdCommand::Left);
    }

    #[test]
    fn right_does_not_send_the_generic_show_command() {
        assert_ne!(command_for_hotkey(Hotkey::Right), OsdCommand::Show);
    }
}
