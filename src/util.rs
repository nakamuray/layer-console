use nix::unistd::{Uid, User};

pub fn get_user_shell() -> String {
    User::from_uid(Uid::current())
        .unwrap()
        .unwrap()
        .shell
        .to_string_lossy()
        .to_string()
}
