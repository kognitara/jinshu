use argon2::Argon2;
use std::fs;

/// Lit la phrase secrète injectée par Systemd et forge la clé AES-256 en RAM
pub fn derive_master_key() -> Result<[u8; 32], String> {
    // 1. Le chemin magique où Systemd dépose le secret (uniquement en RAM)
    let cred_path = "/run/credentials/jinshu.service/master_seed";

    let seed = fs::read_to_string(cred_path)
        .map_err(|_| "Systemd n'a pas fourni le 'master_seed' ! Démarrage avorté.".to_string())?;

    // 2. Le "Sel" (Salt) : empêche les attaques par dictionnaire (Rainbow Tables)
    // Pour l'instant, on met un sel fixe propre à ton moteur.
    let salt = b"jinshu_quantum_core_salt_2026";

    // 3. Configuration d'Argon2 (On prend les paramètres par défaut, très robustes)
    let argon2 = Argon2::default();
    let mut key = [0u8; 32]; // Notre future clé AES-256 vierge

    // 4. La forge mathématique
    argon2
        .hash_password_into(seed.trim().as_bytes(), salt, &mut key)
        .map_err(|e| format!("Échec de la forge Argon2 : {}", e))?;

    // La clé est prête, on la retourne
    Ok(key)
}
use aes_gcm::aead::AeadCore;
use aes_gcm::{
    Aes256Gcm, Key, Nonce,
    aead::{Aead, KeyInit, OsRng},
};

/// Chiffre les octets avec AES-256-GCM de niveau militaire
pub fn chiffrer_aes_256(data: &[u8], secret_key: &[u8; 32]) -> Result<Vec<u8>, String> {
    // 1. Initialisation de la clé
    let key = Key::<Aes256Gcm>::from_slice(secret_key);
    let cipher = Aes256Gcm::new(&key);

    // 2. Génération d'un Nonce aléatoire unique de 96 bits (12 octets)
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    // 3. Chiffrement de la donnée (L'AES-GCM ajoute automatiquement un Tag de sécurité à la fin)
    let ciphertext = cipher
        .encrypt(&nonce, data)
        .map_err(|e| format!("Erreur de chiffrement AES : {:?}", e))?;

    // 4. On concatène : [ NONCE (12 octets) ] + [ CIPHERTEXT + TAG ]
    // On doit sauvegarder le nonce avec la donnée pour pouvoir la déchiffrer !
    let mut final_payload = nonce.to_vec();
    final_payload.extend_from_slice(&ciphertext);

    Ok(final_payload)
}

/// Déchiffre les octets générés par `chiffrer_aes_256`
pub fn dechiffrer_aes_256(payload: &[u8], secret_key: &[u8; 32]) -> Result<Vec<u8>, String> {
    // Sécurité basique : Le payload doit au moins contenir le Nonce (12) + le Tag (16)
    if payload.len() < 28 {
        return Err("Fichier corrompu : payload trop court.".to_string());
    }

    let key = Key::<Aes256Gcm>::from_slice(secret_key);
    let cipher = Aes256Gcm::new(&key);

    // 1. On sépare le Nonce (les 12 premiers octets) du reste de la donnée
    let (nonce_bytes, ciphertext_with_tag) = payload.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    // 2. Déchiffrement et vérification de l'intégrité (Tag GCM)
    let plaintext = cipher.decrypt(nonce, ciphertext_with_tag).map_err(|_| {
        "Déchiffrement impossible : Clé incorrecte ou fichier altéré (Tampering) !".to_string()
    })?;

    Ok(plaintext)
}
