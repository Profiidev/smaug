pub fn permissions() -> Vec<&'static str> {
  vec![
    NodeViewPerm::name(),
    NodeEditPerm::name(),
    SettingsView::name(),
    SettingsEdit::name(),
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
