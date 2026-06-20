use crate::settings;

#[tauri::command]
pub async fn get_api_key_status() -> Result<Vec<settings::KeyStatus>, String> {
    Ok(settings::Settings::load().key_status())
}

#[tauri::command]
pub async fn get_provider_catalog() -> Result<Vec<settings::ProviderCatalogEntry>, String> {
    settings::Settings::load()
        .provider_catalog()
        .map_err(|error| format!("{error:?}"))
}

#[tauri::command]
pub async fn list_provider_models(
    provider: String,
) -> Result<crate::provider_model_catalog::ProviderModelCatalogResult, String> {
    Ok(crate::provider_model_catalog::list_provider_models(&provider).await)
}

#[tauri::command]
pub async fn set_api_key(provider: String, key: String) -> Result<(), String> {
    settings::Settings::load().set_api_key(&provider, &key)
}

#[tauri::command]
pub async fn probe_provider(
    provider: String,
) -> Result<crate::provider_probe::ProviderProbeResult, String> {
    Ok(crate::provider_probe::probe_provider(&provider).await)
}
