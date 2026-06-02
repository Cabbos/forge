use std::sync::Arc;

use crate::agent::delivery_state::DeliveryRecordInput;
use crate::agent::turn_state::AgentTurnState;
use crate::forge_wiki::model::{
    ForgeWikiProposalStatus, ForgeWikiUpdateProposal, SelectedForgeWikiPage,
};
use crate::forge_wiki::storage::ForgeWikiStore;
use crate::forge_wiki::writeback::build_project_archive_writeback;
use crate::state::AppState;
use crate::workflow::{WorkflowRoute, WorkflowState};

pub(crate) struct SendInputProjectRecordsSelection {
    pub(crate) selected: Vec<SelectedForgeWikiPage>,
    pub(crate) context: Option<String>,
}

pub(crate) async fn select_send_input_project_records_context(
    state: &Arc<AppState>,
    text: &str,
    project_path: &str,
) -> SendInputProjectRecordsSelection {
    if !should_select_project_records_for_request(text) {
        return SendInputProjectRecordsSelection {
            selected: Vec::new(),
            context: None,
        };
    }

    match state.forge_wiki.select_context(project_path, text, 4).await {
        Ok(selected) => {
            let context = match state
                .forge_wiki
                .format_selected_context_with_content(project_path, &selected)
            {
                Ok(context) => context,
                Err(error) => {
                    crate::app_log!("WARN", "[forge_wiki] context formatting failed: {}", error);
                    ForgeWikiStore::format_selected_context(&selected)
                }
            };
            SendInputProjectRecordsSelection { selected, context }
        }
        Err(error) => {
            crate::app_log!("WARN", "[forge_wiki] context selection failed: {}", error);
            SendInputProjectRecordsSelection {
                selected: Vec::new(),
                context: None,
            }
        }
    }
}

pub(crate) struct SendInputProjectRecordWriteback {
    pub(crate) proposal: Option<ForgeWikiUpdateProposal>,
    pub(crate) record_evidence: Option<DeliveryRecordInput>,
}

pub(crate) async fn propose_send_input_project_record_update(
    state: &Arc<AppState>,
    session_id: &str,
    text: &str,
    project_path: &str,
    workflow: &WorkflowState,
    latest_turn: Option<&AgentTurnState>,
) -> SendInputProjectRecordWriteback {
    if workflow.route == WorkflowRoute::Direct {
        return SendInputProjectRecordWriteback {
            proposal: None,
            record_evidence: None,
        };
    }

    match state.forge_wiki.get_state(project_path).await {
        Ok(wiki_state) if wiki_state.exists => {
            let Some(writeback) = build_project_archive_writeback(workflow, text, latest_turn)
            else {
                return SendInputProjectRecordWriteback {
                    proposal: None,
                    record_evidence: None,
                };
            };
            match state
                .forge_wiki
                .create_update_proposal(
                    project_path,
                    Some(session_id),
                    writeback.target_pages,
                    writeback.title,
                    writeback.summary,
                )
                .await
            {
                Ok(proposal) => {
                    let record_evidence = if proposal.status == ForgeWikiProposalStatus::Pending {
                        Some(DeliveryRecordInput {
                            status: "pending".to_string(),
                            target_pages: proposal.target_pages.clone(),
                        })
                    } else {
                        None
                    };
                    SendInputProjectRecordWriteback {
                        proposal: Some(proposal),
                        record_evidence,
                    }
                }
                Err(error) => {
                    crate::app_log!("WARN", "[forge_wiki] proposal creation failed: {}", error);
                    SendInputProjectRecordWriteback {
                        proposal: None,
                        record_evidence: None,
                    }
                }
            }
        }
        Ok(_) => SendInputProjectRecordWriteback {
            proposal: None,
            record_evidence: None,
        },
        Err(error) => {
            crate::app_log!("WARN", "[forge_wiki] state check failed: {}", error);
            SendInputProjectRecordWriteback {
                proposal: None,
                record_evidence: None,
            }
        }
    }
}

pub(crate) fn should_select_project_records_for_request(text: &str) -> bool {
    !is_conversation_recall_request(text)
}

fn is_conversation_recall_request(text: &str) -> bool {
    let normalized = text.split_whitespace().collect::<String>();
    if normalized.is_empty() {
        return false;
    }

    let has_recall_topic = [
        "之前说了什么",
        "刚才说了什么",
        "前面说了什么",
        "之前聊了什么",
        "刚才聊了什么",
        "前面聊了什么",
        "聊到哪里",
        "说到哪里",
        "前面讨论",
        "之前讨论",
        "刚才讨论",
        "前面的内容",
        "之前的内容",
        "前面聊的",
        "之前聊的",
    ]
    .iter()
    .any(|signal| normalized.contains(signal));

    let asks_for_summary = normalized.contains("总结")
        || normalized.contains("回顾")
        || normalized.contains("概括")
        || normalized.contains("梳理");
    let references_prior_chat = normalized.contains("之前")
        || normalized.contains("刚才")
        || normalized.contains("前面")
        || normalized.contains("上面")
        || normalized.contains("这段对话");

    has_recall_topic || (asks_for_summary && references_prior_chat)
}

#[cfg(test)]
#[path = "project_records_tests.rs"]
mod tests;
