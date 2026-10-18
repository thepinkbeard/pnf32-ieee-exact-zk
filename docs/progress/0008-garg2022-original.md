# 0008 — Garg 2022 lido no original

**Data:** 2026-10-16 · **Estado:** concluído; B2 confirmada aberta

## 1. Objetivo e estado
Fechar a pendência da rodada 2: ler o Garg, Jain, Jin, Zhang (CCS 2022) no
original e checar propagação de erro, provador malicioso e decisão certificada.

## 2. Arquivos
`docs/refs/garg2022.pdf` (fornecido pelo autor da tese) e
`docs/07-busca-sistematica.md` (adendo).

## 3. Decisões
O veredito "provisório" sobre o Garg em 07 passa a "confirmado". As citações
literais estão no adendo de 07.

## 4. Comandos
`swift scripts/pdftext.swift docs/refs/garg2022.pdf` e grep por termos-chave.

## 5. Evidências
- Garantia só por porta; o end-to-end é delegado a análise numérica externa ou
  a uma suposição heurística.
- O texto admite um provador que escolhe o pior arredondamento.
- Não há decisão certificada; "correct rounding is crucial" aparece como
  trabalho futuro.
- Mystique: 8.854 portas por mul FP32.

## 6. Tentativas malsucedidas
Nenhuma nesta etapa (o PDF veio do autor).

## 7. Limitações
A "full version" citada pelo paper (detalhes de soundness) não foi lida.

## 8. Próximo passo
Protótipo B2: PNF32 + enclosure CBD no mesmo passo Nova, com decisão
trivalorada sobre a saída IEEE.

## 9. Continuidade para outra IA
Use a frase-âncora do adendo de 07 como ponto de partida do posicionamento.
A linha Approx-FP adversarial do paper do autor é a evidência empírica contra o modelo do Garg em caos.
