def is_palindrome(s: str) -> bool:
    """检查字符串是否为回文（忽略大小写和空格）。"""
    # 去空格并转小写
    cleaned = "".join(s.split()).lower()
    return cleaned == cleaned[::-1]


if __name__ == "__main__":
    test_cases = [
        ("racecar", True),
        ("hello", False),
        ("A man a plan a canal Panama", True),
        ("上海自来水来自海上", True),
        ("Python", False),
        ("", True),
    ]

    all_pass = True
    for text, expected in test_cases:
        result = is_palindrome(text)
        status = "PASS" if result == expected else "FAIL"
        if result != expected:
            all_pass = False
        print(f"{status}: is_palindrome({text!r}) = {result} (expected {expected})")

    print()
    if all_pass:
        print("所有测试通过！")
    else:
        print("存在失败的测试！")
        exit(1)
