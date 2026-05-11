def fibonacci(n):
    """返回前 n 个斐波那契数"""
    if n <= 0:
        return []
    elif n == 1:
        return [0]
    seq = [0, 1]
    for i in range(2, n):
        seq.append(seq[-1] + seq[-2])
    return seq


if __name__ == "__main__":
    print("斐波那契数列前 15 项：")
    print(fibonacci(15))
