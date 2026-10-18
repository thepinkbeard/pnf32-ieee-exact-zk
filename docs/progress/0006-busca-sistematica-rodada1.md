# 0006 — Busca sistemática na literatura, rodada 1

**Data:** 2026-10-16 · **Estado:** concluído (provisório; rodada 2 pendente)

## 1. Objetivo e estado
Checar se as brechas candidatas B1–B5 já foram ocupadas. Resultado:
B1 ocupada (ZKLP), B3 com teoria ocupada (pay-per-bit), B2 aberta (candidata
a núcleo da tese), B4 e B5 parcialmente abertas.

## 2. Arquivos
`docs/07-busca-sistematica.md` (novo), `docs/01-literatura.md` (nota de correção no topo).

## 3. Decisões
- Reposicionar o PNF32 como bloco de construção sem lookups para folding,
  citando o ZKLP como estado da arte de IEEE exato.
- Propor B2 (prova dupla L2+L3) como núcleo, a confirmar na rodada 2.

## 4. Comandos
Cerca de 15 buscas web e leitura do ZKLP (HTML do arXiv, Tabela I e Tabela II).

## 5. Evidências
ZKLP Tabela I (R1CS/op, FP32): add 42+43, mul 31+33, div 38+38, sqrt 23+22, mais
~291 fixas de tabela. Tabela II: aplicação com 19,5 mil constraints (Groth16)
contra 309,6 mil do baseline P20.

## 6. Tentativas malsucedidas
PDFs do ePrint (403) e o PDF da NYU/OSDI não foram legíveis. Não há
`pdftotext`/`poppler` na máquina; a saída foi usar as versões HTML do arXiv.

## 7. Limitações
Busca só na web, sem snowballing de citações. Vereditos provisórios.

## 8. Próximo passo
Rodada 2 focada em B2 (lista em 07, seção "Rodada 2").

## 9. Continuidade para outra IA
Não reivindicar novidade em "IEEE barato". Antes de escrever qualquer coisa
sobre B2, ler o Garg 2022 completo (propagação de erro) e o ZIP.
