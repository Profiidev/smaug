pub fn permissions() -> Vec<&'static str> {
  vec![NodeView::name(), NodeEdit::name()]
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

permission!(NodeView, "node:view");
permission!(NodeEdit, "node:edit");
