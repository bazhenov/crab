# Crab

Parser-toolkit for crawling and parsing sites on the Internet. The main goal is to provide tools for implementing
independent crawling and parsing.

## Why independent crawling matters?

Nearly all parsers are written using try and error approach. If crawling and parsing are done together you are forced
to download documents from Internet each time you change a parser. Crab tries to separate crawling logic from
parsing logic and maintain localy stored mirror of pages you are parsing.

## Architecture

```mermaid
flowchart LR
  cli
  navigator
  parser
  tabulator
  internet((Internet))
  validator

  subgraph db
    pages[(Pages)]
    links[(Links)]
    kv[(KV-storage)]
  end

  tables[(Tables)]

  crawler --> pages
  internet --> crawler --> validator --> navigator --> links --> pages
  cli --> crawler
  pages --> parser --> kv --> tabulator --> tables
```