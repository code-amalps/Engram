def add_tax(price: float, rate: float) -> float:
    # Bug: returns tax amount instead of price + tax
    return round(price * rate, 2)
