use crate::action::Action;
use crate::binding::BindingId;
use crate::binding::BindingOptions;
use crate::binding::Passthrough;
use crate::key::Hotkey;

pub(crate) struct RegisteredBinding {
    id: BindingId,
    hotkey: Hotkey,
    action: Action,
    options: BindingOptions,
}

impl RegisteredBinding {
    #[must_use]
    pub(crate) fn new(id: BindingId, hotkey: Hotkey, action: Action) -> Self {
        Self {
            id,
            hotkey,
            action,
            options: BindingOptions::default(),
        }
    }

    #[must_use]
    pub(crate) fn with_options(mut self, options: BindingOptions) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub(crate) fn with_passthrough(mut self, passthrough: Passthrough) -> Self {
        self.options = self.options.with_passthrough(passthrough);
        self
    }

    #[must_use]
    pub(crate) const fn id(&self) -> BindingId {
        self.id
    }

    #[must_use]
    pub(crate) fn hotkey(&self) -> &Hotkey {
        &self.hotkey
    }

    #[must_use]
    pub(crate) const fn action(&self) -> &Action {
        &self.action
    }

    #[must_use]
    pub(crate) const fn passthrough(&self) -> Passthrough {
        self.options.passthrough()
    }

    #[must_use]
    pub(crate) const fn options(&self) -> &BindingOptions {
        &self.options
    }
}
