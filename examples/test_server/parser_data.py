from bs4 import BeautifulSoup

TYPE_ID: int = 2

def parse(content: str) -> dict[str, str]:
    html = BeautifulSoup(content, 'html.parser')
    data = {}

    input = html.select_one(".input")
    output = html.select_one(".output")

    if input and output:
        data["input"] = input.text
        data["output"] = output.text

    return data
