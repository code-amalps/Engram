def safe_divide(values):
    # Bug: crashes with ZeroDivisionError when values sum to 0 or list is empty
    total = sum(values)
    return [v / total for v in values]
