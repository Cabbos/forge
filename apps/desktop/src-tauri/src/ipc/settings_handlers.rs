use crate::settings;
use crate::state::AppState;
use std::sync::Arc;

#[tauri::command]
pub async fn get_api_key_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<settings::KeyStatus>, String> {
    Ok(settings::Settings::load().key_status(state.credential_store.as_ref()))
}

#[tauri::command]
pub async fn get_provider_catalog() -> Result<Vec<settings::ProviderCatalogEntry>, String> {
    settings::Settings::load()
        .provider_catalog()
        .map_err(|error| format!("{error:?}"))
}

#[tauri::command]
pub async fn list_provider_models(
    state: tauri::State<'_, Arc<AppState>>,
    provider: String,
) -> Result<crate::provider_model_catalog::ProviderModelCatalogResult, String> {
    let profile = state.profiles.get_active_profile();
    crate::provider_model_catalog::list_provider_models(
        &provider,
        &state.credential_resolver(),
        profile.as_ref(),
    )
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_api_key(
    state: tauri::State<'_, Arc<AppState>>,
    provider: String,
    key: String,
) -> Result<(), String> {
    settings::Settings::load().set_api_key(state.credential_store.as_ref(), &provider, &key)
}

#[tauri::command]
pub async fn upsert_provider_profile(
    input: settings::ProviderProfileInput,
) -> Result<settings::ProviderCatalogEntry, String> {
    let mut settings = settings::Settings::load();
    settings.upsert_provider_profile(input)
}

#[tauri::command]
pub async fn delete_provider_profile(provider: String) -> Result<(), String> {
    let mut settings = settings::Settings::load();
    settings.delete_provider_profile(&provider)
}

#[tauri::command]
pub async fn probe_provider(
    state: tauri::State<'_, Arc<AppState>>,
    provider: String,
) -> Result<crate::provider_probe::ProviderProbeResult, String> {
    let profile = state.profiles.get_active_profile();
    crate::provider_probe::probe_provider(&provider, &state.credential_resolver(), profile.as_ref())
        .await
        .map_err(|error| error.to_string())
}
