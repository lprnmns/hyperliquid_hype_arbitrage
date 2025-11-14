//! EIP-712 Signing Module for Hyperliquid
//! 
//! Handles cryptographic signing of orders and actions using EIP-712 standard

use anyhow::{Result, Context};
use ethers::{
    signers::{LocalWallet, Signer as EthersSigner},
    types::{Address, Signature, H256},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, info};
use hex;
use sha3::{Keccak256, Digest};

// Hyperliquid constants
const CHAIN_ID: u64 = 42161; // Arbitrum
const DOMAIN_NAME: &str = "Hyperliquid";
const DOMAIN_VERSION: &str = "1";
const VERIFYING_CONTRACT: &str = "0x0000000000000000000000000000000000000000"; // Update with actual

/// Signer for Hyperliquid orders
#[derive(Clone)]
pub struct Signer {
    wallet: LocalWallet,
    address: Address,
}

impl Signer {
    /// Create a new signer from private key
    pub fn new(private_key: &str) -> Result<Self> {
        let wallet = private_key
            .parse::<LocalWallet>()
            .context("Failed to parse private key")?;
        
        let address = wallet.address();
        
        info!("Signer initialized for address: {:?}", address);
        
        Ok(Signer {
            wallet,
            address,
        })
    }
    
    /// Sign an action for Hyperliquid
    pub async fn sign_action(
        &self,
        action: Value,
        nonce: u64,
        vault_address: Option<String>,
    ) -> Result<String> {
        // Create the message to sign
        let message = OrderAction {
            action,
            nonce,
            vault_address,
        };
        
        // Create typed data
        let typed_data = self.create_typed_data(&message)?;
        
        // Sign the typed data
        let signature = self.sign_typed_data(&typed_data).await?;
        
        // Convert to hex string
        let sig_hex = format!("0x{}", hex::encode(signature.to_vec()));
        
        debug!("Signature generated: {}", sig_hex);
        
        Ok(sig_hex)
    }
    
    /// Create typed data for EIP-712 signing
    fn create_typed_data(&self, message: &OrderAction) -> Result<TypedData> {
        let typed_data = TypedData {
            domain: self.create_domain(),
            primary_type: "OrderAction".to_string(),
            types: self.create_types(),
            message: serde_json::to_value(message)?,
        };
        
        Ok(typed_data)
    }
    
    /// Create domain separator
    fn create_domain(&self) -> HashMap<String, Value> {
        let mut domain = HashMap::new();
        domain.insert("name".to_string(), Value::String(DOMAIN_NAME.to_string()));
        domain.insert("version".to_string(), Value::String(DOMAIN_VERSION.to_string()));
        domain.insert("chainId".to_string(), Value::Number(CHAIN_ID.into()));
        domain.insert("verifyingContract".to_string(), Value::String(VERIFYING_CONTRACT.to_string()));
        domain
    }
    
    /// Create type definitions for EIP-712
    fn create_types(&self) -> HashMap<String, Vec<TypeField>> {
        let mut types = HashMap::new();
        
        // Define EIP712Domain type
        types.insert(
            "EIP712Domain".to_string(),
            vec![
                TypeField {
                    name: "name".to_string(),
                    r#type: "string".to_string(),
                },
                TypeField {
                    name: "version".to_string(),
                    r#type: "string".to_string(),
                },
                TypeField {
                    name: "chainId".to_string(),
                    r#type: "uint256".to_string(),
                },
                TypeField {
                    name: "verifyingContract".to_string(),
                    r#type: "address".to_string(),
                },
            ],
        );
        
        // Define OrderAction type
        types.insert(
            "OrderAction".to_string(),
            vec![
                TypeField {
                    name: "action".to_string(),
                    r#type: "bytes".to_string(),
                },
                TypeField {
                    name: "nonce".to_string(),
                    r#type: "uint64".to_string(),
                },
                TypeField {
                    name: "vaultAddress".to_string(),
                    r#type: "address".to_string(),
                },
            ],
        );
        
        types
    }
    
    /// Sign typed data using the wallet
    async fn sign_typed_data(&self, typed_data: &TypedData) -> Result<Signature> {
        // Create the hash to sign
        let hash = self.hash_typed_data(typed_data)?;
        
        // Sign the hash
        let signature = self.wallet.sign_hash(H256::from_slice(&hash))?;
        
        Ok(signature)
    }
    
    /// Hash typed data according to EIP-712
    fn hash_typed_data(&self, typed_data: &TypedData) -> Result<Vec<u8>> {
        // For now, use a simplified version
        // In production, use full EIP-712 implementation
        let mut hasher = Keccak256::new();
        hasher.update(serde_json::to_vec(typed_data)?);
        Ok(hasher.finalize().to_vec())
    }
    
    /// Get the signer's address
    pub fn address(&self) -> Address {
        self.address
    }
    
    /// Verify a signature
    pub fn verify_signature(
        &self,
        message: &[u8],
        signature: &str,
    ) -> Result<bool> {
        let sig_bytes = hex::decode(signature.trim_start_matches("0x"))?;
        let sig = Signature::try_from(sig_bytes.as_slice())?;
        
        let recovered = sig.recover(message)?;
        Ok(recovered == self.address)
    }
}

/// Order action structure for signing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderAction {
    pub action: Value,
    pub nonce: u64,
    #[serde(rename = "vaultAddress")]
    pub vault_address: Option<String>,
}

/// Type field for EIP-712
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeField {
    pub name: String,
    pub r#type: String,
}

/// Simplified typed data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedData {
    pub domain: HashMap<String, Value>,
    #[serde(rename = "primaryType")]
    pub primary_type: String,
    pub types: HashMap<String, Vec<TypeField>>,
    pub message: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_signer_creation() {
        let private_key = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let signer = Signer::new(private_key);
        assert!(signer.is_ok());
    }
    
    #[tokio::test]
    async fn test_signing() {
        let private_key = "0x0000000000000000000000000000000000000000000000000000000000000001";
        let signer = Signer::new(private_key).unwrap();
        
        let action = serde_json::json!({
            "type": "order",
            "orders": []
        });
        
        let signature = signer.sign_action(action, 12345, None).await;
        assert!(signature.is_ok());
        assert!(signature.unwrap().starts_with("0x"));
    }
}