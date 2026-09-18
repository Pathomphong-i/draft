use super::error::ObjectError;

/// A Blob represents uninterpreted, binary-safe sequence of raw bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Blob {
    data: Vec<u8>,
}

impl Blob {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    pub fn from_slice(slice: &[u8]) -> Self {
        Self {
            data: slice.to_vec(),
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn serialize(&self) -> &[u8] {
        &self.data
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, ObjectError> {
        Ok(Self::new(data.to_vec()))
    }
}
