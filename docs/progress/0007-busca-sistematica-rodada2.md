# 0007 — Busca sistemática, rodada 2 (prova dupla L2+L3)

**Data:** 2026-10-16 · **Estado:** concluído; B2 segue aberta

## 1. Objetivo e estado
Atacar B2 a fundo: Garg 2022 completo, ZIP completo, buscas dirigidas e
snowballing. Resultado: nenhum trabalho faz L2+L3. A área de certificação de
redes neurais documenta o gap real × float como ataque, o que reforça a motivação.

## 2. Arquivos
`docs/07-busca-sistematica.md` (seção "Rodada 2").

## 3. Decisões
- B2 vira a formulação candidata da tese (*dual-semantics verifiable computation*).
- Garg 2022 não pôde ser lido no original; o veredito sobre ele é provisório.

## 4. Comandos
- WebSearch (6 consultas dirigidas).
- Leitura do ZIP: PDF do VTechWorks extraído com PDFKit
  (`scripts/pdftext.swift`; a máquina não tem poppler).
- OpenAlex `filter=cites:W4411337825` (ZKLP) e `cites:W4308642742` (Garg).

## 5. Evidências
Ver a tabela "O que foi lido" e a lista de citantes em 07. PANDA declara
explicitamente que não é sound para float.

## 6. Tentativas malsucedidas
- par.nsf.gov: ECONNREFUSED via WebFetch, curl e navegador embutido.
- ACM DL: 403.
- Semantic Scholar API: 429.

## 7. Limitações
Garg 2022 lido só via terceiros. Sem Google Scholar. Áreas novas (2026) mudam rápido.

## 8. Próximo passo
1. Obter o PDF do Garg 2022 (biblioteca/orientador) e conferir a propagação de erro.
2. Protótipo B2: PNF32 + enclosure CBD no mesmo passo Nova, com decisão
   trivalorada sobre a saída IEEE; medir o custo.

## 9. Continuidade para outra IA
Para extrair texto de PDF nesta máquina, use Swift/PDFKit
(`print(PDFDocument(url:).string)`). OpenAlex sem chave tem cota diária pequena.
