/// Host-local connection generation. Never accept this value as authentication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Connection(u64);

#[derive(Debug, Eq, PartialEq)]
pub enum AdmissionError {
    Busy,
    GenerationExhausted,
}

/// Instantiate once per running host. A single host event loop owns admission.
/// A release notification from an old connection cannot release a newer one.
#[derive(Default)]
pub struct Admission {
    generation: u64,
    active: Option<Connection>,
}

impl Admission {
    /// Call only after successful authentication and required local approval.
    pub fn admit(&mut self) -> Result<Connection, AdmissionError> {
        if self.active.is_some() {
            return Err(AdmissionError::Busy);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(AdmissionError::GenerationExhausted)?;
        let connection = Connection(self.generation);
        self.active = Some(connection);
        Ok(connection)
    }

    pub fn is_active(&self, connection: Connection) -> bool {
        self.active == Some(connection)
    }

    /// Release after capture/input shutdown and key-release cleanup completes.
    pub fn release(&mut self, connection: Connection) -> bool {
        if !self.is_active(connection) {
            return false;
        }
        self.active = None;
        true
    }
}
