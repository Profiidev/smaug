pub fn permissions() -> Vec<&'static str> {
  vec![
    NodeViewPerm::name(),
    NodeEditPerm::name(),
    SettingsView::name(),
    SettingsEdit::name(),
    GroupView::name(),
    GroupEdit::name(),
    UserView::name(),
    UserEdit::name(),
  ]
}

pub trait Permission {
  fn name() -> &'static str;
}

macro_rules! permission {
  ($type:ident, $name:literal) => {
    pub struct $type;

    impl Permission for $type {
      fn name() -> &'static str {
        $name
      }
    }
  };
}

// No permissions required
permission!(NoPerm, "");

// Nodes
permission!(NodeViewPerm, "node:view");
permission!(NodeEditPerm, "node:edit");

// Settings
permission!(SettingsView, "settings:view");
permission!(SettingsEdit, "settings:edit");

// Groups
permission!(GroupView, "group:view");
permission!(GroupEdit, "group:edit");

// Users
permission!(UserView, "user:view");
permission!(UserEdit, "user:edit");
