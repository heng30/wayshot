mod about;
mod clipboard;
mod confirm_dialog;
mod popup_action;
mod setting;
mod toast;
mod tr;
mod util;

mod camera;
mod downloader;
mod history;
mod player;
mod push_stream;
mod realtime_image_effect;
mod recorder;
mod share_screen;
mod update_dialog;
mod video_editor;

pub use video_editor::project::{
    BG_ANIMATION_CONFIG_ID, CODE_IMAGE_CONFIG_ID, GLOBAL_FILTER_CONFIG_ID, TEXT_STYLE_CONFIG_ID,
    TTFX_CONFIG_ID,
};

pub fn init(ui: &crate::slint_generatedAppWindow::AppWindow) {
    util::init(ui);
    clipboard::init(ui);
    about::init(ui);
    setting::init(ui);

    toast::init(ui);
    confirm_dialog::init(ui);
    popup_action::init(ui);
    downloader::init(ui);
    update_dialog::init(ui);

    recorder::init(ui);
    history::init(ui);
    player::init(ui);
    camera::init(ui);
    push_stream::init(ui);
    realtime_image_effect::init(ui);

    video_editor::init(ui);
    share_screen::init(ui);
}

#[macro_export]
macro_rules! global_store {
    ($ui:expr) => {
        $ui.global::<crate::slint_generatedAppWindow::Store>()
    };
}

#[macro_export]
macro_rules! global_logic {
    ($ui:expr) => {
        $ui.global::<crate::slint_generatedAppWindow::Logic>()
    };
}

#[macro_export]
macro_rules! global_util {
    ($ui:expr) => {
        $ui.global::<crate::slint_generatedAppWindow::Util>()
    };
}

#[macro_export]
macro_rules! global_ve_filter {
    ($ui:expr) => {
        $ui.global::<crate::slint_generatedAppWindow::VEFilter>()
    };
}

#[macro_export]
macro_rules! global_terminal_state {
    ($ui:expr) => {
        $ui.global::<crate::slint_generatedAppWindow::TerminalState>()
    };
}

#[macro_export]
macro_rules! logic_cb {
    ($callback_name:ident, $ui:expr, $($arg:ident),*) => {
        {{
            let ui_weak = $ui.as_weak();
            paste::paste! {
                crate::global_logic!($ui)
                    .[<on_ $callback_name>](move |$($arg),*| {
                        $callback_name(&ui_weak.unwrap(), $($arg),*)
                    });
            }
        }}
    };
    ($callback_name:ident, $ui:expr) => {
        {{
            let ui_weak = $ui.as_weak();
            paste::paste! {
                crate::global_logic!($ui)
                    .[<on_ $callback_name>](move || {
                        $callback_name(&ui_weak.unwrap())
                    });
            }
        }}
    };
}

#[macro_export]
macro_rules! logic_cb_pure {
    ($callback_name:ident, $ui:expr, $($arg:ident),*) => {
        {{
            let ui_weak = $ui.as_weak();
            paste::paste! {
                crate::global_logic!($ui)
                    .[<on_ $callback_name>](move |$($arg),*| {
                        $callback_name(&ui_weak.unwrap(), $($arg),*)
                    });
            }
        }}
    };
    ($callback_name:ident, $ui:expr) => {
        {{
            let ui_weak = $ui.as_weak();
            paste::paste! {
                crate::global_logic!($ui)
                    .[<on_ $callback_name>](move || {
                        $callback_name(&ui_weak.unwrap())
                    });
            }
        }}
    };
}

#[macro_export]
macro_rules! terminal_state_cb {
    ($callback_name:ident, $ui:expr, $($arg:ident),*) => {
        {{
            let ui_weak = $ui.as_weak();
            paste::paste! {
                crate::global_terminal_state!($ui)
                    .[<on_ $callback_name>](move |$($arg),*| {
                        $callback_name(&ui_weak.unwrap(), $($arg),*)
                    });
            }
        }}
    };
    ($callback_name:ident, $ui:expr) => {
        {{
            let ui_weak = $ui.as_weak();
            paste::paste! {
                crate::global_terminal_state!($ui)
                    .[<on_ $callback_name>](move || {
                        $callback_name(&ui_weak.unwrap())
                    });
            }
        }}
    };
}

#[macro_export]
macro_rules! impl_slint_enum_serde {
    ($ty:ident, $($arg:ident),+) => {
        impl serde::Serialize for $ty {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                match self {
                    $(
                        $ty::$arg => serializer.serialize_str(stringify!($arg)),
                    )+
                }
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                struct EnumVisitor;

                impl<'de> serde::de::Visitor<'de> for EnumVisitor {
                    type Value = $ty;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str(&format!("a string representing {}", stringify!($ty)))
                    }

                    fn visit_str<E>(self, value: &str) -> Result<$ty, E>
                    where
                        E: serde::de::Error,
                    {
                        match value {
                            $(
                                stringify!($arg) => Ok($ty::$arg),
                            )+
                            _ => Err(E::custom(format!(
                                "unknown {} variant: {}",
                                stringify!($ty),
                                value
                            ))),
                        }
                    }
                }

                deserializer.deserialize_str(EnumVisitor)
            }
        }
    };
}

// Example: impl_c_like_enum_convert!(Foo, Bar, A, B, C);
#[macro_export]
macro_rules! impl_c_like_enum_convert {
    ($enum1:ident, $enum2:ident, $($variant:ident),*) => {
        impl From<$enum1> for $enum2 {
            fn from(value: $enum1) -> Self {
                match value {
                    $(
                        $enum1::$variant => $enum2::$variant,
                    )*
                }
            }
        }

        impl From<$enum2> for $enum1 {
            fn from(value: $enum2) -> Self {
                match value {
                    $(
                        $enum2::$variant => $enum1::$variant,
                    )*
                }
            }
        }
    };
}

#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use crate::impl_slint_enum_serde;

    #[derive(Debug, Clone)]
    enum MyEnum {
        VariantA,
        VariantB,
    }

    impl_slint_enum_serde!(MyEnum, VariantA, VariantB);

    // cargo test test_slint_enum_serde -- --no-capture
    #[test]
    fn test_impl_slint_enum_serde() {
        let va = serde_json::to_string(&MyEnum::VariantA).unwrap();
        let vb = serde_json::to_string(&MyEnum::VariantB).unwrap();
        println!("{}", va);
        println!("{}", vb);

        let va = serde_json::from_str::<MyEnum>(&va).unwrap();
        println!("{:?}", va);
    }
}
