# 00 — Pergunta e hipóteses

## Problema

O paper *Impact of Numeric Representation on Verifiable Semantics in
Zero-Knowledge Blockchain Systems* mostra, no mapa logístico caótico
(`r = 3,9`), que codificações baratas (Q16.16, Q8.8, quantização de 8 bits,
Approx-FP) divergem da referência FP32 em poucas dezenas de passos e produzem
mais de 440 inversões do predicado `x >= 0,5` em 1.000 passos. A saída óbvia, que
é provar o próprio IEEE 754, é tida como cara: na literatura, cerca de 2.456
constraints para uma soma FP32 bit a bit (Garg et al.). No repositório
`proof-numeric-zk`, o passo FP32 exato no Nova custa **1.721** constraints,
contra 709 do CBD-32, que nem é bit-exato.

## Pergunta

> Existe uma representação numérica **pelo menos tão fiel quanto o IEEE 754**
> (aqui: bit-exata com binary32 RNE, incluindo subnormais, ±0, ±inf e NaN) que
> gere **menos constraints** que as alternativas não bit-exatas?

Decisões tomadas com o autor antes de implementar:

- fidelidade = **bit-exata com FP32** (os mesmos bits do SoftFloat/hardware);
- escopo = **binary32 geral desde a v1**, não só positivo-normal.

## Hipótese central

O custo do IEEE em R1CS vem principalmente da **aritmetização** e não do
formato: range checks largos e repetidos, alinhamento na escala absoluta
`2^-149`, revalidação de valores já validados. Uma codificação desempacotada
com relações *hint-and-verify* de largura fixa deve reduzir o custo sem mudar
um único bit do resultado.

## Hipóteses pré-registradas (fixadas no plano aprovado, antes de medir)

| | hipótese | critério |
|---|---|---|
| H1 | operações individuais baratas | mul ≤ 150 e add/sub ≤ 250 constraints, binary32 geral |
| H2 | passo logístico bit-exato barato | `1-x`, `x·c`, `p·r` + decisão < 1.003 (CFD); meta forte ≤ 709 (CBD-32) |
| H3 | fidelidade | trajetória bit-idêntica ao f32 em r=3,5 e 3,9, N=1.000; zero flips |
| H4 | soundness | zero witnesses alternativos aceitos na enumeração exaustiva (formato mini) |

**Go/no-go:** go se H2 (forma fraca), H3 e H4 valerem. Um resultado negativo seria
registrado como tal, sem enfraquecer o perfil IEEE para ganhar números.

Resultado: ver [06-resultados.md](06-resultados.md).
