use super::{Chunks, ChunksError};

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    ChunksError(ChunksError),
    InvalidPayload,
}

/// A type for reading server-sent events from a chunk stream.
pub struct Sse {
    buf: String,
    chunks: Chunks,
}

impl Sse {
    #[inline]
    pub fn new(chunks: Chunks) -> Self {
        Self {
            buf: String::new(),
            chunks,
        }
    }

    pub async fn next_event(&mut self) -> Result<Option<String>, Error> {
        loop {
            // Read more data from the stream first.
            let mut has_more_data = false;
            if let Some(bytes) =
                self.chunks.next_chunk().await.map_err(Error::ChunksError)?
            {
                let Ok(s) = str::from_utf8(&bytes) else {
                    return Err(Error::InvalidPayload);
                };
                self.buf.push_str(s);
                has_more_data = true;
            }

            // There are data in the buffer, try to parse an event. If the data
            // is not enough to parse an event, we need to read more.
            if let Some(event) = self.try_parse_event()? {
                return Ok(Some(event));
            }

            // Abort if no more data available.
            if !has_more_data {
                return Ok(None);
            }
        }
    }

    fn try_parse_event(&mut self) -> Result<Option<String>, Error> {
        if self.buf.is_empty() {
            return Ok(None);
        }

        // For `end-of-line`, we only handle line feed. And for event, we
        // only handle field.
        //
        // event         = *( comment / field ) end-of-line
        // field         = 1*name-char [ colon [ space ] *any-char ] end-of-line
        // end-of-line   = ( cr lf / cr / lf )
        let Some(eol_idx) = self.buf.find("\n\n") else {
            return Ok(None);
        };

        // Parse the field line.
        let field = &self.buf[0..eol_idx];
        let mut field_parts = field.split(": ");
        let Some(header) = field_parts.next() else {
            return Err(Error::InvalidPayload);
        };
        if header != "data" {
            // Other events are not supported.
            return Err(Error::InvalidPayload);
        }
        let Some(data) = field_parts.next() else {
            return Err(Error::InvalidPayload);
        };
        let data = data.to_owned();

        // Consume the bytes from the buffer.
        self.buf.drain(0..eol_idx + 2);

        Ok(Some(data))
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;

    #[tokio::test]
    async fn test_normal_events() {
        let chunks = Chunks::from_vec_deque(
            vec![
                Bytes::from_static(b"data: hello\n\n"),
                Bytes::from_static(b"data: bye\n\n"),
            ]
            .into(),
        );
        let mut sse = Sse::new(chunks);
        assert_eq!(sse.next_event().await.unwrap().unwrap(), "hello");
        assert_eq!(sse.next_event().await.unwrap().unwrap(), "bye");
        assert_eq!(sse.next_event().await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_quirk_streaming() {
        let chunks = Chunks::from_vec_deque(
            vec![
                Bytes::from_static(b"data:"),
                Bytes::from_static(b" hello\n"),
                Bytes::from_static(b"\n"),
            ]
            .into(),
        );
        let mut sse = Sse::new(chunks);
        assert_eq!(sse.next_event().await.unwrap().unwrap(), "hello");
        assert_eq!(sse.next_event().await.unwrap(), None);
    }

    #[tokio::test]
    async fn test_invalid_data() {
        let chunks = Chunks::from_vec_deque(
            vec![Bytes::from_static(b"xxxxxx\n\n")].into(),
        );
        let mut sse = Sse::new(chunks);
        assert_eq!(sse.next_event().await.unwrap_err(), Error::InvalidPayload);

        let chunks = Chunks::from_vec_deque(
            vec![Bytes::from_static(b"xxxxxx\n")].into(),
        );
        let mut sse = Sse::new(chunks);
        assert_eq!(sse.next_event().await.unwrap(), None);

        let chunks = Chunks::from_vec_deque(
            vec![
                Bytes::from_static(b"data: hello\n"),
                Bytes::from_static(b"data: bye\n"),
            ]
            .into(),
        );
        let mut sse = Sse::new(chunks);
        assert_eq!(sse.next_event().await.unwrap(), None);
    }
}
