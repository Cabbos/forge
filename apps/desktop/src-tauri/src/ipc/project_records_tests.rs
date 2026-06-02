use crate::ipc::project_records::should_select_project_records_for_request;

#[test]
fn conversation_recall_requests_do_not_auto_inject_project_records() {
    assert!(!should_select_project_records_for_request(
        "我们之前说了什么"
    ));
    assert!(!should_select_project_records_for_request(
        "刚才聊到哪里了？"
    ));
    assert!(!should_select_project_records_for_request(
        "总结一下前面讨论过的内容"
    ));

    assert!(should_select_project_records_for_request(
        "继续优化当前项目的首页"
    ));
    assert!(should_select_project_records_for_request(
        "根据项目记录看看下一步"
    ));
}
