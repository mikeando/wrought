use std::{io::{Read, Write}, net::TcpStream};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};

use anyhow::{anyhow, Context as _};

use super::embedding::{Embedding, EmbeddingError};


#[derive(Debug)]
struct VectorValue {
    vectors: Vec<f32>,
}

#[derive(Debug)]
struct VectorError {
    error_code: u32,
    error_message: String
}

enum VectorResponse {
    VectorValue(VectorValue),
    VectorError(VectorError),
}



pub struct EmbeddingClient {
    pub host: String,
    pub port: u16,

    // TODO: We should get this info from the server.
    max_token_count: usize,
    embedding_length: usize
}

#[derive(Debug)]
pub struct EmbeddingInfo {
    max_token_count: u32,
    dimension: u32,
}

pub enum GetEmbeddingInfoResponse {
    Ok(EmbeddingInfo),
    Err(VectorError),
}

impl EmbeddingClient {

    pub fn create(host: &str, port: u16) -> anyhow::Result<EmbeddingClient> {
        //TODO: Connect and get the max_token_count and embedding_length from 
        //      the server.
        // This is a dumb hack - but it works for now
        let temp_client = EmbeddingClient{host: host.to_string(), port, max_token_count:0, embedding_length:0};
        println!("Getting info...");
        let embedding_info = temp_client.get_embedding_info()?;
        println!("info = {:?}", embedding_info);

        let max_token_count = embedding_info.max_token_count as usize;
        let embedding_length = embedding_info.dimension as usize;
        Ok(EmbeddingClient{host: host.to_string(), port, max_token_count, embedding_length})
    }

    pub fn embed(&self, bytes: &[u8]) -> anyhow::Result<Vec<f32>> {
        let request = self.create_vectorize_request(bytes);
        let mut stream = TcpStream::connect((self.host.as_str(), self.port)).context("connection error")?;
        stream.write_all(&request).context("send error")?;

        let mut response_data = Vec::new();
        stream.read_to_end(&mut response_data).context("read error")?;
        drop(stream);

        let response = self.parse_response(&response_data)?;

        match response {
            VectorResponse::VectorValue(vector_value) => {
                Ok(vector_value.vectors)
            },
            VectorResponse::VectorError(vector_error) => {
                Err(anyhow!("Error: {}", vector_error.error_message))
            },
        }
    }

    pub fn get_embedding_info(&self) -> anyhow::Result<EmbeddingInfo> {
        let request = self.create_get_embedding_info_request();
        let mut stream = TcpStream::connect((self.host.as_str(), self.port)).context("connection error")?;
        stream.write_all(&request).context("send error")?;

        let mut response_data = Vec::new();

        stream.read_to_end(&mut response_data).context("read error")?;
        drop(stream);

        let response = self.parse_get_embedding_info_response(&response_data)?;
        match response {
            GetEmbeddingInfoResponse::Ok(v) => {
                Ok(v)
            },
            GetEmbeddingInfoResponse::Err(vector_error) => {
                Err(anyhow!("Error: {}", vector_error.error_message))
            },
        }
    }

    fn create_vectorize_request(&self, bytes: &[u8]) -> Vec<u8> {
        let mut request = Vec::new();
        let command_id = 1;
        let prototocol_id = b"TV01";

        request.extend_from_slice(prototocol_id);    
        request.write_u32::<BigEndian>(command_id).unwrap();
        request.write_u64::<BigEndian>(bytes.len() as u64).unwrap();
        request.extend_from_slice(bytes);
        
        request
    }

    fn create_get_embedding_info_request(&self) -> Vec<u8> {
        let mut request = Vec::new();
        let command_id = 2;
        let prototocol_id = b"TV01";
        request.extend_from_slice(prototocol_id);    
        request.write_u32::<BigEndian>(command_id).unwrap();
        request
    }

    fn parse_response(&self, response_data: &[u8]) -> anyhow::Result<VectorResponse> {
        if response_data.len() < 8 {  // Minimum size: 4 bytes flag + 4 bytes count/error code
            return Err(anyhow!("response too short"));
        }
    
        let mut cursor = &response_data[..];
        
        // Read success flag (4 bytes)
        let ok_value = cursor.read_u32::<BigEndian>().context("unable to parse status flag")?;
        let ok = ok_value == 1;
    
        if !ok {
            return Ok(VectorResponse::VectorError(self.parse_error_response(cursor)?))
        } 

        // Parse success response - read vector count (4 bytes)
        let vector_count = cursor.read_u32::<BigEndian>().context("unable to read result count")? as usize;
        let mut vectors = Vec::with_capacity(vector_count);
        
        // Read each f32 in big-endian order
        for i in 0..vector_count {
            let value = cursor.read_f32::<BigEndian>().context("unable to read result component")?;
            vectors.push(value);
        }
        
        Ok(VectorResponse::VectorValue(VectorValue { vectors }))
    }

    fn parse_error_response(&self, cursor: &[u8]) -> anyhow::Result<VectorError> {
        let mut cursor = &cursor[..];

        // Parse error response
        let error_code = cursor.read_u32::<BigEndian>().context("unable to read error code")?;
        let error_len = cursor.read_u64::<BigEndian>().context("unable to read error length")? as usize;
        let error_message = String::from_utf8(cursor[..error_len].to_vec()).context("unable to convert error message to UTF-8")?;
        
        Ok(VectorError{
            error_code,
            error_message})
    }

    fn parse_get_embedding_info_response(&self, response_data: &[u8]) -> anyhow::Result<GetEmbeddingInfoResponse> {
        let mut cursor = &response_data[..];
        
        // Read success flag (4 bytes)
        let ok_value = cursor.read_u32::<BigEndian>().context("unable to parse status flag")?;
        let ok = ok_value == 1;

        if !ok {
            return Ok(GetEmbeddingInfoResponse::Err(self.parse_error_response(cursor)?))
        }

        // The max tokens and dimension are both u32s
        let max_token_count = cursor.read_u32::<BigEndian>().context("unable to parse max token count")?;
        let dimension =  cursor.read_u32::<BigEndian>().context("unable to parse dimension")?;

        Ok(
            GetEmbeddingInfoResponse::Ok(EmbeddingInfo { max_token_count, dimension })
        )

    }
}

impl Embedding for EmbeddingClient {
    fn estimeated_token_count(&self, data: &[u8]) -> usize {
        todo!()
    }

    fn calculate_embedding(&self, data: &[u8]) -> Result<Vec<f32>, EmbeddingError> {
        Ok(self.embed(data)?)
    }

    fn max_token_count(&self) -> usize {
        self.max_token_count
    }

    fn embedding_length(&self) -> usize {
        self.embedding_length
    }
}

impl From<anyhow::Error> for EmbeddingError {
    fn from(value: anyhow::Error) -> Self {
        EmbeddingError::Other(value)
    }
}