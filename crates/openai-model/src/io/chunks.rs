#[cfg(test)]
use std::collections::VecDeque;

use bytes::Bytes;
use reqwest::Response;

#[derive(Debug, PartialEq, Eq)]
pub struct Error;

/// An adapter for streaming byte chunks.
pub enum Chunks {
    Response(Response),
    #[cfg(test)]
    VecDeque(VecDeque<Bytes>),
}

impl Chunks {
    pub fn from_response(response: Response) -> Self {
        Chunks::Response(response)
    }

    #[cfg(test)]
    pub fn from_vec_deque(vec: VecDeque<Bytes>) -> Self {
        Chunks::VecDeque(vec)
    }

    #[inline]
    pub async fn next_chunk(&mut self) -> Result<Option<Bytes>, Error> {
        match self {
            Chunks::Response(response) => {
                let Ok(chunk) = response.chunk().await else {
                    return Err(Error);
                };
                Ok(chunk)
            }
            #[cfg(test)]
            Chunks::VecDeque(vec) => {
                let chunk = vec.pop_front();
                Ok(chunk)
            }
        }
    }
}
