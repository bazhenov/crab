from bs4 import BeautifulSoup
import parser_data

TYPE_ID: int = 1

def navigate(content: str) -> list[(str, int)]:
    html = BeautifulSoup(content, 'html.parser')
    links = []
    for a in html.select("section.pager a"):
        link = a.attrs['href']
        links.append((link, TYPE_ID))

    for a in html.select("ul a"):
        link = a.attrs['href']
        links.append((link, parser_data.TYPE_ID))
    return links