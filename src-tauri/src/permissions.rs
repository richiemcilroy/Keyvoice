use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub enum PermissionState {
    NotNeeded,
    NotRequested,
    Granted,
    Denied,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Permission {
    pub state: PermissionState,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct Permissions {
    pub microphone: Permission,
    pub accessibility: Permission,
}

impl Permissions {
    pub fn check() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::check_macos()
        }
        #[cfg(target_os = "windows")]
        {
            Self::check_windows()
        }
        #[cfg(target_os = "linux")]
        {
            Self::check_linux()
        }
    }

    #[cfg(target_os = "macos")]
    fn check_macos() -> Self {
        use crate::platform::macos::permissions::{
            check_accessibility_permission, check_microphone_permission,
        };

        Permissions {
            microphone: Permission {
                state: check_microphone_permission(),
                name: "Microphone".to_string(),
            },
            accessibility: Permission {
                state: check_accessibility_permission(),
                name: "Accessibility".to_string(),
            },
        }
    }

    #[cfg(target_os = "windows")]
    fn check_windows() -> Self {
        Permissions {
            microphone: Permission {
                state: PermissionState::NotNeeded,
                name: "Microphone".to_string(),
            },
            accessibility: Permission {
                state: PermissionState::NotNeeded,
                name: "Accessibility".to_string(),
            },
        }
    }

    #[cfg(target_os = "linux")]
    fn check_linux() -> Self {
        Permissions {
            microphone: Permission {
                state: PermissionState::NotNeeded,
                name: "Microphone".to_string(),
            },
            accessibility: Permission {
                state: PermissionState::NotNeeded,
                name: "Accessibility".to_string(),
            },
        }
    }

    pub fn request_permission(permission_type: &str) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            use crate::platform::macos::permissions::request_permission;
            request_permission(permission_type)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = permission_type;
            Ok(())
        }
    }
}
