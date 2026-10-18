# 07 — Busca sistemática na literatura (rodada 1)

**Data:** 2026-10-16. **Objetivo:** verificar se as cinco brechas candidatas
listadas no fim da sessão de resultados já foram ocupadas por trabalhos
publicados.

## Protocolo

- **Fontes:** busca web (IACR ePrint, arXiv, ACM DL, USENIX, IEEE S&P, blogs técnicos
  de auditoria) em cerca de 15 consultas, com leitura direta do texto quando o
  PDF ou o HTML estava acessível (ZKLP pelo arXiv HTML; Sridhar, ZIP e
  sum-check aproximado pelo resumo do ePrint).
- **Consultas por brecha:** IEEE 754 exato em ZK; intervalos e erro certificado em ZK;
  modelo de custo de folding e valores pequenos; detecção de circuitos
  subrestritos; compiladores e perfis numéricos para ZK.
- **Limites desta rodada:** não houve busca em Google Scholar/DBLP com
  snowballing de citações, e alguns PDFs (OSDI NYU, ZIP completo) não puderam
  ser lidos. Os vereditos são **provisórios**; "não encontrado" significa
  "não encontrado nesta rodada", não "não existe".

## Correções ao que estava em 01-literatura e no paper

1. **Garg et al. (CCS 2022)** são *Sanjam Garg, Abhishek Jain, Zhengzhong Jin,
   Yinuo Zhang*. O esquema deles prova um **limite de erro relativo por porta**:
   o provador honesto segue o IEEE, mas a prova só certifica a relaxação, e um
   provador malicioso pode escolher o pior arredondamento dentro do limite.
   Eles relatam ~57× sobre "o método que segue o IEEE exatamente" em FP32.
2. **ZKLP (Ernstberger et al., IEEE S&P 2025, pp. 3440–3459)** já mostra IEEE
   754 **completo** (subnormais, ±0, ±inf, NaN, RNE, testado contra Berkeley
   TestFloat) barato e **em R1CS**. Ele usa gnark (Groth16 e Plonk) e LogUp
   embutido no circuito. Tabela I deles, constraints R1CS por operação FP32:
   add/sub 42 nativas + 43 de lookup ≈ **85**; mul 31 + 33 ≈ **64**; div 38+38;
   sqrt 23+22. Somam-se ~291 constraints fixas de tabela, amortizadas. Sem
   amortização, 2 multiplicações custam 209 constraints (~105 cada).
   **Consequência:** a premissa "IEEE exato é caro" já estava refutada na
   literatura quando o seu paper foi escrito, e o próprio paper cita os 64 do
   ZKLP. O PNF32 (mul 134, add 145, sem lookups) é de 1,6× a 2,1× **mais caro**
   que o ZKLP por operação.

## Veredito por brecha

| # | brecha | estado da arte encontrado | veredito |
|---|---|---|---|
| B1 | IEEE bit-exato barato | **ZKLP** (S&P 2025): 64–85 constraints/op em R1CS com LogUp. Mystique (USENIX Sec 2021) e trabalhos citados: ~8.000 por mul. ZIP (Riasi et al., CCS 2025): FP64 com lookups, foco em zkML | **ocupada.** Sobra para o PNF32: variante **sem lookups**, adequada a folding/IVC (Nova não tem lookup nativo), com validade como invariante indutivo. Contribuição de engenharia, não de tese |
| B2 | prova dupla L2+L3: "é exatamente o FP32" **e** "eis o limite certificado até a matemática exata e a robustez da decisão" | Garg: erro relativo por porta, sem propagação para a trajetória real e sem exigir o IEEE exato. Sum-check aproximado (Bitan, DeStefano, Goldwasser, Ishai, Kalai, Thaler, EUROCRYPT 2026): aceita somas ≈ com tolerância, sem IEEE nem intervalos. ZKLP/ZIP: só L2. Fora de ZK: análise de erro certificada (PRECiSA, Rosa/Daisy, FPTaylor) gera certificados checáveis em Coq/PVS, mas não em SNARK | **aberta nesta rodada. É a candidata mais forte** |
| B3 | contagem de constraints não prevê o tempo do prover em folding | Explicação conhecida: o custo de commitment é **pay-per-bit**, e valores pequenos ficam 10–20× mais baratos (NeutronNova 2024/1606; Mova 2024/1220; Neo/SuperNeo; HyperNova) | **teoria ocupada.** O seu achado é uma consequência medida dela. Serve como argumento metodológico ("compare representações numéricas por bits largos comprometidos, não por constraints"), não como contribuição central |
| B4 | verificação de unicidade do witness por enumeração completa em formatos reduzidos paramétricos | Picus/QED² (determinismo via SMT), Ecne, Circomspect, zkFuzz (S&P 2026, fuzzing com TCCT). Nenhum trabalho com checagem limitada em bit-width reduzido para gadgets numéricos | **parcialmente aberta.** É um análogo da *small-scope hypothesis* (Alloy) para circuitos ZK numéricos. Contribuição metodológica secundária |
| B5 | perfis numéricos verificáveis (especificação → circuito → verificação) | Peças isoladas: "rounding gates" em provas interativas (IEEE Access 2022); Rosa/Daisy sintetizam float/fixo a partir de reais com especificação de erro, fora de ZK. Nada encontrado ponta a ponta para ZK | **aberta**, mas com grande peso de engenharia. Bom como "veículo" da tese, fraco como pergunta científica isolada |

## Leitura para a tese

1. **Não reivindicar** "IEEE exato barato" como novidade: o ZKLP já fez, com
   menos constraints. O PNF32 deve ser apresentado como **bloco de construção**
   (versão sem lookup para folding), citando o ZKLP como estado da arte.
2. **Núcleo candidato:** B2. Uma prova que certifica **ao mesmo tempo**
   - (a) a execução bit-exata da semântica de referência do software (FP32), e
   - (b) um envelope racional certificado da trajetória matemática exata, com a
     decisão marcada como *robusta* (as duas concordam e o envelope não cruza o
     limiar) ou *indeterminada*.

   Isso conecta os três níveis do seu paper (L1 soundness, L2 fidelidade, L3
   aplicação) numa única afirmação verificável. Nada parecido apareceu nesta
   rodada. Você já tem as duas metades implementadas no mesmo backend (PNF32 e
   CBD/CDAD).
3. **Métodos de apoio:** B4 (unicidade em formato reduzido) como contribuição
   metodológica; B3 como recomendação de métrica de avaliação.

## Rodada 2 (antes de comprometer a tese com B2)

- Ler o texto completo de **Garg et al.** (seção de propagação de erro: eles
  compõem limites ao longo do circuito?) e de **ZIP** (o "relative-error-driven
  technique" certifica algo contra a matemática exata?).
- Buscas dirigidas: "certified error bound" + SNARK/zkSNARK; "interval arithmetic"
  + "zero-knowledge"; FPTaylor/PRECiSA/Daisy + "verifiable computation";
  "verifiable numerical analysis"; "robust decision" + zkML.
- Snowballing: quem cita o ZKLP e o Garg 2022 (Google Scholar "cited by").
- Ler o paper DCHPC 2026 *Towards Trustworthy Digital Twins: Verifiable
  Simulation via Recursive ZK Proofs* (Homaei et al.): simulação verificável com
  recursão, possivelmente próximo de B2 no workload (não foi possível ler o resumo).

## Fontes

- ZKLP: [arXiv 2404.14983](https://arxiv.org/abs/2404.14983) · [ePrint 2024/1842](https://eprint.iacr.org/2024/1842)
- Garg, Jain, Jin, Zhang, CCS 2022: [PAR/NSF](https://par.nsf.gov/biblio/10408517) · [ACM](https://dl.acm.org/doi/abs/10.1145/3548606.3560653)
- ZIP (Riasi et al., CCS 2025): [ePrint 2025/1732](https://eprint.iacr.org/2025/1732)
- Sridhar et al., USENIX Sec 2026: [ePrint 2025/2326](https://eprint.iacr.org/2025/2326)
- Sum-check aproximado, EUROCRYPT 2026: [ePrint 2025/2152](https://eprint.iacr.org/2025/2152)
- Mystique, USENIX Sec 2021: [ePrint 2021/730](https://eprint.iacr.org/2021/730)
- NeutronNova: [ePrint 2024/1606](https://eprint.iacr.org/2024/1606.pdf) · Mova: [ePrint 2024/1220](https://eprint.iacr.org/2024/1220.pdf) · Neo/SuperNeo: [Springer](https://link.springer.com/chapter/10.1007/978-3-032-35424-2_17)
- zkFuzz, S&P 2026: [arXiv 2504.11961](https://arxiv.org/abs/2504.11961) · comparação de ferramentas: [zkSecurity](https://blog.zksecurity.xyz/posts/zksecurity-tools/) · Picus no RISC Zero: [blog](https://risczero.com/blog/RISCZero-formally-verified-zkvm)
- Erro certificado fora de ZK: [On Sound Relative Error Bounds (FMCAD'17)](https://arxiv.org/pdf/1707.02121) · [A Verified Certificate Checker for FP Error Bounds](https://deepai.org/publication/a-verified-certificate-checker-for-floating-point-error-bounds)
- Digital twins verificáveis, DCHPC 2026: [UNIR](https://investigacion.unir.net/documentos/6a37b46042e7be579416fe6b)

---

# Rodada 2 — foco em B2 (prova dupla L2+L3)

**Data:** 2026-10-16. Registro: [0007](progress/0007-busca-sistematica-rodada2.md).

## O que foi lido

| fonte | acesso | o que importa para B2 |
|---|---|---|
| **ZIP** (Riasi, Wang, Behnia, Vo, Hoang, CCS 2025) | texto completo (PDF do VTechWorks) | O servidor calcula a ativação em IEEE FP64 e usa esse valor adiante, mas a prova só mostra que ele está a um erro **relativo δ de uma aproximação polinomial por partes** (escolhida por lookup privado). Não prova que o valor é o IEEE bit-exato (não é L2) nem dá um limite até a matemática exata ao fim do pipeline (não é L3). Camadas lineares usam técnica de [20]. |
| **Garg, Jain, Jin, Zhang** (CCS 2022) | PDF inacessível (par.nsf.gov recusou conexão; ACM 403). Lido via descrições de citantes e do resumo | Prova **erro relativo por operação** contra o resultado real das entradas *daquela* operação. Reconhecem que o circuito pode ser "aproximadamente satisfazível nos reais, mas não nos floats" e esperam que essa folga seja pequena. Segundo o ZKLP, cobrem só add/mul e não têm implementação prática. **Não** há indício de propagação até a matemática exata nem de decisão certificada. ⚠ Confirmar lendo o PDF (pedir ao orientador ou à biblioteca). |
| **PANDA**, *Certified but Private* (arXiv 2608.17070, ago/2026) | HTML | ZK de certificados CROWN (robustez/fairness) até 2,9 M parâmetros. Aritmética **quantizada** de 14 bits. Os autores declaram: *"can only prove robustness for quantized models, not for general floating-point models"* e medem a deriva contra a aritmética exata só empiricamente. **Nem L2 (float) nem L3 sound.** É o trabalho mais próximo em espírito ("provar uma garantia, não só uma execução"). |
| Sum-check aproximado (Bitan et al., EUROCRYPT 2026) | resumo | Tolerância δ no protocolo, sem IEEE nem intervalos. Outra camada (o protocolo, não a semântica). |
| *ZK verification for frontier AI training* (arXiv 2606.05433) | resumo | Observa que a prova verifica uma computação diferente da que a GPU executa (não associatividade do float). Mesmo diagnóstico do seu paper, sem solução L2+L3. |

## Evidência fora de ZK (motivação forte para B2)

A comunidade de **certificação de redes neurais** documentou em 2022–2026 que
certificados válidos nos reais **falham** na execução em ponto flutuante:

- *Getting a-Round Guarantees: Floating-Point Attacks on Certified Robustness*
  ([arXiv 2205.10159](https://arxiv.org/pdf/2205.10159));
- *No Soundness in the Real World* ([arXiv 2506.01054](https://arxiv.org/html/2506.01054)):
  todos os verificadores testados atacáveis na implantação;
- Tobler, Syeda, Murray, *A Formally Verified Robustness Certifier* (CAV 2025,
  [arXiv 2505.06958](https://arxiv.org/html/2505.06958)): um certificador
  publicado "certifica" tudo por erro de arredondamento;
- *Lipschitz-Based Robustness Certification Under Floating-Point Execution*
  ([arXiv 2603.13334](https://arxiv.org/pdf/2603.13334)): teoria
  composicional do gap real × float.

É exatamente a separação L2/L3 do seu paper, só que vista por outra área. A
ponte para ZK não foi encontrada.

## Snowballing (OpenAlex, `cites:`)

- **Citam o ZKLP (7):** proof-of-location veicular; Eva (S&P 2025, vídeo);
  ScaleCirc (ASE 2025, análise de Circom); banking/ORAM; sum-check aproximado;
  segurança de IA em veículos autônomos (ICCPS 2026); Celer (lookup). **Nenhum** faz L2+L3.
- **Citam o Garg 2022 (13):** ZK proofs of training (CCS 2023, ponto fixo);
  ZKROWNN; ZKLP; ZkVML; Game of Coding (×3, tolerância em oráculos); MPC-in-the-head;
  ZIP; FREPack; sum-check aproximado; pkCNN. **Nenhum** faz L2+L3.

## Veredito da rodada 2

**B2 continua aberta, agora com evidência mais forte de que é relevante.**

1. Nenhum trabalho encontrado prova, numa mesma afirmação verificável, (a) a
   execução **bit-exata** da semântica float de referência **e** (b) um
   **envelope certificado** até a matemática exata, com a decisão marcada como
   robusta ou indeterminada.
2. Os trabalhos mais próximos ocupam **uma metade cada**:
   ZKLP/PNF32 → (a); Garg → (b) **local** e sem soundness end-to-end;
   PANDA → certificado de propriedade, mas quantizado e explicitamente não sound
   para float; seu CBD/CDAD → (b) end-to-end, sem (a).
3. A área vizinha (certificação de redes) já reconhece o problema real × float
   como ataque prático. Isso dá a **motivação de segurança** que o seu paper já
   usa (L1 ≠ L3).

**Formulação candidata do problema da tese:**

> *Dual-semantics verifiable computation*: provar em zero conhecimento que a
> saída é exatamente a que a implementação IEEE produziria **e** que a decisão
> derivada dela é (ou não) robusta em relação à computação matemática exata,
> com custo medido e soundness verificada.

**Riscos remanescentes:**

- O PDF do Garg 2022 não foi lido. Se houver propagação de erro composicional
  com decisão, B2 se estreita (improvável pelas descrições, mas é obrigatório
  conferir).
- O ZIP não foi checado nos apêndices.
- Busca só em inglês e em fontes indexadas na web. Falta Google Scholar com
  "cited by" do PANDA e do *Lipschitz under FP* (área nova, de 2026, que pode
  ter trabalhos concorrentes).
- Novidade da formulação ≠ dificuldade técnica. O protótipo (PNF32 + CBD no
  mesmo passo) precisa mostrar algo não trivial: custo, composição com
  folding, ou a decisão trivalorada certificada sobre a saída IEEE.

---

# Adendo — leitura do Garg 2022 no original

PDF fornecido pelo autor da tese: `docs/refs/garg2022.pdf` (CC-BY 4.0).
Registro: [0008](progress/0008-garg2022-original.md). As citações são do PDF
(páginas da versão CCS).

**1. O que a prova certifica: só erro local, por porta.**
> "we only verify an upper bound on the relative error for each floating point
> operation in the overall computation" (p. 1203)

A relação é `|c − g(a,b)| ≤ δ|g(a,b)|`, onde `a, b` são os valores (já
arredondados) das próprias entradas da porta. **Não há propagação** até a
saída nem limite contra a computação matemática exata. A garantia
end-to-end é terceirizada: "a proof in our model can be combined with
numerical analysis about the program to obtain guarantees on the accuracy of
the output" (p. 1204). Para programas sem análise conhecida: "we can
**heuristically assume** that their output is robust to any small perturbations" (p. 1204).

**2. Provador malicioso: admitido explicitamente.**
> "a malicious prover may choose any floating point numbers that lead to the
> maximum possible rounding error in the worst case" (p. 1205)

A soundness é a noção fraca: rejeitar só se **não existir** atribuição real com
erro relativo ≤ δ em cada porta. Reconhecem um "theoretical gap" entre
completude e soundness e esperam que ele seja "rather narrow in practice"
(seção da construção).

**3. Decisão certificada: inexistente.** Também não há intervalos nem
predicados. Trabalho futuro declarado: "constructing efficient succinct proof
systems for applications where **the correct rounding is crucial**, such as
finance and accounting" (Conclusão).

**Custo de referência citado:** Mystique [35] converte uma mul FP32 em
**8.854 portas** binárias (p. 1203).

## Consequências para a tese

- **B2 confirmada como aberta** em relação ao Garg: eles cobrem só (b)
  local, delegam o end-to-end a uma análise externa e não certificam decisões.
- **A hipótese heurística deles é refutável no seu próprio experimento.**
  "Assume-se que o programa é robusto a perturbações pequenas" é falso em
  regime caótico. A linha *Approx-FP adversarial* do seu paper (r=3,9: primeiro
  flip em n=26, 491 flips) é exatamente o provador malicioso que o Garg admite,
  explorando o δ por porta. O seu paper já tem a evidência de que o modelo de
  erro relativo **não** basta para L3 em dinâmicas sensíveis.
- **O trabalho futuro que eles declaram** ("correct rounding is crucial") é o
  que o ZKLP fez e o que o PNF32 faz em folding. A combinação com o envelope
  certificado vai além dos dois.
- Frase-âncora possível para a introdução da tese: *per-operation error
  models (Garg et al.) leave end-to-end guarantees to external numerical
  analysis and admit worst-case adversarial rounding; exact-IEEE circuits
  (ZKLP) certify the software's result but say nothing about its distance to
  the intended mathematics. We prove both in one statement.*
