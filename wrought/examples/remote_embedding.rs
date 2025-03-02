use wrought::kb::remote_embedding::EmbeddingClient;
use wrought::kb::embedding::Embedding;

use anyhow::anyhow;

fn main() -> anyhow::Result<()> {
    // Connect to the service
    let client: Box<dyn Embedding> = Box::new(EmbeddingClient::create("localhost", 5555)?);

    println!("The embedding we're using has a max token count of {}", client.max_token_count());
    println!("and embedds into a dimension of {}", client.embedding_length());
    
    // Create and send request
    
    // Read response
    let texts = &[
        "Hello from Rust!", 
        "I love to dance.", 
        "Ponies like dancing too.",
        "Greetings from C++."];
    let mut vecs: Vec<Vec<f32>> = vec![];
    for text in texts {
        let vectors = client.calculate_embedding(text.as_bytes()).map_err(|e| anyhow!("eep: {:?}", e))?;
    
        // display results
        println!("Successfully vectorized text: '{}'", text);
        println!("Vector dimension: {}", vectors.len());
        println!("First 5 values: {:.6?}", &vectors[..5]);
        
        // Calculate some basic statistics
        let sum: f32 = vectors.iter().sum();
        let mean = sum / vectors.len() as f32;
        println!("Mean value: {:.6}", mean);
        
        let magnitude: f32 = vectors.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!("Vector magnitude: {:.6}", magnitude);

        vecs.push(vectors);
    }

    let dp = |a: &Vec<f32>, b: &Vec<f32>| -> f32 {
        a.iter().zip(b.iter()).map(|(aa, bb)| aa*bb).sum::<f32>()
    };

    for v1 in &vecs {
        for v2 in &vecs {
            eprint!("{} ", dp(v1,v2));
        }
    }

    // Now lets see what happens as we increase the length of the string.
    let mut s = "".to_string();
    for i in 0..500 {
        if i != 0 {
            s += " "
        }
        s += texts[i % texts.len()];
        if i % 10 == 0 {
            let _v = client.calculate_embedding(s.as_bytes()).map_err(|e| anyhow!("eep2: {:?}", e))?;
            eprintln!("{} Vectorised text of length {}", i, s.len());
        }
    }
    
    Ok(())
}