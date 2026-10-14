# Fase 1: Fundação Técnica — Plano de Implementação

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Trocar a fundação do Sofinance (TypeScript, rotas de verdade, cache de dados, design system e banco versionado) sem mudar nada que o usuário veja, para que as Fases 2 e 3 sejam construídas sobre base sólida.

**Architecture:** O app continua sendo uma SPA React servida pela Vercel, com Supabase como backend. A mudança é estrutural: o roteamento sai de `useState` e vai para o React Router com URLs reais; a busca de dados sai do `useEffect` por tela e vai para o TanStack Query; as regras puras saem de `src/utils` e viram `src/domain` tipado; o CSS artesanal ganha Tailwind v4 e shadcn/ui ao lado (convivendo, não substituindo de uma vez); e o banco passa a ter migrations versionadas com um espelho local rodando em Docker. Componentes existentes continuam em `.jsx` e são convertidos aos poucos, exceto a tela piloto.

**Tech Stack:** React 19.3, Vite 8.3, TypeScript 5.9.3, React Router 8.4, TanStack Query 5.103, Tailwind CSS 4.3, shadcn/ui (base Radix), Vitest 5, ESLint 10 + typescript-eslint 8.70, Supabase CLI 2.117, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-17-sofinance-revamp-design.md` (seções 5.1, 7 Fase 1, 13.2).

---

## Decisões já tomadas (com o motivo)

Estas foram verificadas contra o registro do npm em 2026-09-23. Não as reabra sem um motivo novo.

| Decisão                                                                     | Motivo verificado                                                                                                                                                                                                                                                                                                                             |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | ---------- |
| **TypeScript 5.9.3, não 7.0**                                               | `typescript-eslint` (inclusive a versão canary 8.70.2-alpha.5) declara `peerDependencies.typescript: ">=4.8.4 <6.1.0"`. Com TypeScript 7 o lint com informação de tipos para de funcionar. Reavaliar quando o typescript-eslint publicar suporte.                                                                                             |
| **React 19.3**                                                              | `react-router@8.4.0` exige `react >=19.2.7`. Sem React 19 não há React Router 8. Todas as outras bibliotecas do projeto já aceitam React 19.                                                                                                                                                                                                  |
| **lucide-react 1.x**                                                        | A versão instalada (0.263.1) declara peer `react` só até 18. É obrigatório subir junto com o React 19.                                                                                                                                                                                                                                        |
| **Vite 8.3 + @vitejs/plugin-react 6.1 + Vitest 5**                          | `@vitejs/plugin-react@6` exige `vite ^8`, e `vitest@5` exige `vite ^6.4                                                                                                                                                                                                                                                                       |     | ^7  |     | ^8`. Os peers extras do plugin (`oxc-transform-react`, `@rolldown/plugin-babel`, `babel-plugin-react-compiler`) são todos `optional: true`, então a instalação é limpa. Node instalado é 24.19, acima do mínimo do Vite 8 (`^20.19 |     | >=22.12`). |
| **shadcn/ui com base Radix, não Base UI**                                   | `@base-ui-components/react` ainda está em `1.0.0-rc.0`. Radix é estável hoje. Reavaliar na Fase 3; o shadcn permite trocar a base depois.                                                                                                                                                                                                     |
| **Recharts continua na 2.x**                                                | A 2.15.4 já aceita React 19. A migração para a 3.x acontece na Fase 3, junto com o redesenho dos gráficos.                                                                                                                                                                                                                                    |
| **Sonner fica para a Fase 3**                                               | Trocar `react-hot-toast` agora mexeria em 7 telas sem ganho nesta fase.                                                                                                                                                                                                                                                                       |
| **Componentes continuam `.jsx`**                                            | A migração para TypeScript nesta fase cobre `domain`, `lib`, `services`, o roteador, os providers e a tela piloto. O resto é convertido quando for reescrito na Fase 2.                                                                                                                                                                       |
| **`src/lib` e `src/components/ui`, não `src/shared/lib` e `src/shared/ui`** | O spec (seção 5.1) sugeria `src/shared/*`, mas o shadcn/ui espera `@/lib/utils` e `@/components/ui` por padrão. Brigar com o padrão da ferramenta custaria configuração extra em toda instalação de componente, sem ganho. O papel das pastas é o mesmo que o spec descreve.                                                                  |
| **`date-fns` sai do projeto**                                               | Está instalado na versão 2.30 e é importado apenas em `DashboardHome.jsx`, onde os imports (`format`, `ptBR`) não são usados. Todas as datas já passam por `src/domain/dates.ts`, que usa `Date` nativo. Remover em vez de atualizar.                                                                                                         |
| **`actions/checkout@v7` e `actions/setup-node@v7`**                         | Conferido na API do GitHub em 2026-09-25: as releases atuais sao checkout v7.0.1 e setup-node v7.0.0, e as tags moveis `v7` existem nas duas. A v7 do setup-node nao mexe em `node-version` nem em `cache: npm`. O plano trazia `@v5`, escrito de memoria e ja dois majors vencido — versao que nao passa por esta tabela nao foi verificada. |

---

## Global Constraints

- Branch de trabalho: `fase-1/fundacao`, criada a partir de `main`. Nada vai para `main` antes da Task 13.
- Datas: nunca usar `new Date().toISOString().split('T')[0]` nem `new Date('YYYY-MM-DD')`; usar `src/domain/dates.ts`.
- Dinheiro continua `number` em reais nesta fase. A conversão para centavos inteiros é da Fase 2.
- Toda regra de negócio pura vive em `src/domain/` como função pura com teste. Nada de regra em componente.
- Nenhuma tabela, view ou função nova em produção nesta fase, com uma exceção: a migration de `default privileges` da Task 1.
- `npm run lint`, `npm run typecheck`, `npm run test:run` e `npm run build` devem passar ao final de cada task.
- Commits em português, no formato `tipo: descrição` (`feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `build:`), terminando com a linha `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- Nunca escrever senha, token ou chave em arquivo versionado. A senha do banco é passada por variável de ambiente `SUPABASE_DB_PASSWORD` ou digitada na hora; ela não entra em nenhum arquivo.
- O projeto Supabase de produção é `ctchgljqbardmzajgyjp`. Nenhuma task altera dados de produção.

---

## Mapa de arquivos

| Caminho                                                                 | Responsabilidade                                                                                  |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `supabase/migrations/20260917000000_baseline_remote_schema.sql` (novo)  | Retrato do schema de produção, gerado pelo `db pull`; é o ponto de partida de qualquer banco novo |
| `supabase/migrations/20260923000000_default_privileges_anon.sql` (novo) | Impede que tabelas futuras nasçam acessíveis ao papel `anon`                                      |
| `tsconfig.json`, `tsconfig.node.json` (novos)                           | Configuração do TypeScript para o app e para os arquivos de build                                 |
| `src/vite-env.d.ts` (novo)                                              | Tipos das variáveis `import.meta.env`                                                             |
| `src/types/database.types.ts` (novo, gerado)                            | Tipos das tabelas do Supabase; nunca editado à mão                                                |
| `vite.config.ts` (substitui `vite.config.js` e `vitest.config.js`)      | Build, alias `@`, Tailwind e configuração dos testes num arquivo só                               |
| `eslint.config.js`                                                      | Lint com TypeScript, React e acessibilidade                                                       |
| `.github/workflows/ci.yml` (novo)                                       | Lint, typecheck, testes e build a cada push e PR                                                  |
| `src/domain/*.ts` (movidos de `src/utils`)                              | Regras puras: moeda, datas, vencimentos, resumo, payload de transação, validações                 |
| `src/lib/logger.ts`, `src/lib/errorHandler.ts` (movidos)                | Infraestrutura: log e tradução de erro                                                            |
| `src/lib/supabase.ts` (move `src/services/supabaseClient.js`)           | Cliente Supabase tipado                                                                           |
| `src/lib/queryClient.ts` (novo)                                         | Configuração única do TanStack Query                                                              |
| `src/lib/utils.ts` (novo, criado pelo shadcn)                           | Helper `cn` para classes                                                                          |
| `src/app/router.tsx` (novo)                                             | Definição das rotas                                                                               |
| `src/app/providers.tsx` (novo)                                          | Providers em um lugar só                                                                          |
| `src/app/RequireAuth.tsx` (novo)                                        | Guarda de rota autenticada                                                                        |
| `src/app/AppLayout.tsx` (novo)                                          | Moldura com sidebar e `<Outlet />`                                                                |
| `src/services/*.ts` (convertidos)                                       | Acesso ao Supabase, tipado                                                                        |
| `src/features/contas-fixas/` (novo)                                     | Tela piloto: `api.ts`, `queries.ts`, `ContasFixasPage.tsx`                                        |
| `src/components/ui/` (novo, shadcn)                                     | Componentes de interface reutilizáveis                                                            |
| `CLAUDE.md` (novo)                                                      | Instruções permanentes do projeto para o Claude Code                                              |
| `.claude/skills/sofinance-domain/SKILL.md` (novo)                       | Regras de negócio consultáveis por qualquer agente                                                |

---

### Task 1: Banco local e migrations versionadas — CONCLUÍDA em 2026-09-23 (commit `466a2c6`)

**O que deu diferente do previsto, para quem precisar refazer:**

1. **A CLI exige a senha por variável de ambiente nesta máquina.** O método automático dela (criar um papel temporário `cli_login_postgres`) é recusado pelo projeto com `permission denied to alter role`. Solução: `set -a; source .env; set +a` antes de qualquer comando, com `SUPABASE_DB_PASSWORD` no `.env`.
2. **O `db pull` não roda com as migrations da Fase 0 presentes.** Ele monta um banco sombra aplicando as migrations locais do zero, e a primeira delas altera tabelas que ninguém criou. Sequência que funciona: mover as migrations existentes para fora da pasta, `migration repair --status reverted` nelas, rodar `db pull`, renomear o arquivo gerado para `20260917000000_baseline_remote_schema.sql`, devolver as migrations e `migration repair --status applied` em todas.
3. **Descoberta não prevista: CRLF dentro do corpo das funções.** O `db diff` acusava as cinco funções para sempre. Causa: `core.autocrlf=true` fazia o Git escrever CRLF no disco, e o `\r` foi parar dentro do corpo das funções em produção quando o SQL original foi aplicado. Correção em duas partes: `.gitattributes` com `* text=auto eol=lf` (causa raiz) e a migration `20260918000200_normalizar_funcoes_lf.sql`, que reescreve as mesmas definições com LF. Depois disso o `db diff` responde `No schema changes found`.

**Estado final:** cinco migrations, todas aplicadas local e remotamente, banco reconstruível do zero com `npm run db:reset`.

### Task 1 (texto original)

**Files:**

- Create: `supabase/migrations/20260917000000_baseline_remote_schema.sql` (gerado), `supabase/migrations/20260923000000_default_privileges_anon.sql`
- Modify: `package.json` (scripts), `.gitignore`

**Interfaces:**

- Produces: `npm run db:start`, `db:stop`, `db:reset`, `db:diff`, `db:types` funcionando; histórico de migrations coerente entre local e produção.

**Contexto que o brief não sabe:** as duas migrations existentes (`20260917230000_fase0_rls_lockdown.sql` e `20260918000100_transacoes_colunas_cartao.sql`) foram aplicadas em produção pela Management API, não pela CLI, então a tabela `supabase_migrations.schema_migrations` do servidor não as conhece. Elas são idempotentes de propósito, então rodar de novo sobre o baseline não quebra nada.

- [ ] **Step 1: Conferir que o Docker está de pé**

```bash
docker --version
docker info --format "{{.ServerVersion}}"
npx supabase --version
```

Expected: versão do Docker e do servidor sem erro, e CLI 2.117 ou maior. Se `docker info` falhar com "cannot connect", abra o Docker Desktop e espere o ícone ficar verde antes de seguir.

- [ ] **Step 2: Registrar as duas migrations da Fase 0 como já aplicadas em produção**

```bash
npx supabase migration repair --status applied 20260917230000 20260918000100
```

Expected: `Repaired migration history`. Isso não executa SQL, só ajusta o histórico.

- [ ] **Step 3: Puxar o retrato do schema de produção**

```bash
npx supabase db pull
```

Quando pedir a senha do banco, use a que está no painel do Supabase em Project Settings → Database. Não escreva a senha em nenhum arquivo.

Expected: cria `supabase/migrations/<timestamp_de_hoje>_remote_schema.sql` com os `CREATE TABLE`, views e funções de produção.

- [ ] **Step 4: Renomear o baseline para vir antes das migrations da Fase 0**

O arquivo gerado tem a data de hoje, ou seja, ordenaria depois das migrations que alteram essas mesmas tabelas. Num banco novo isso quebraria.

```bash
cd supabase/migrations
mv *_remote_schema.sql 20260917000000_baseline_remote_schema.sql
cd ../..
ls supabase/migrations
```

Expected: exatamente três arquivos, nesta ordem: `20260917000000_baseline_remote_schema.sql`, `20260917230000_fase0_rls_lockdown.sql`, `20260918000100_transacoes_colunas_cartao.sql`.

- [ ] **Step 5: Registrar o baseline no histórico remoto**

```bash
npx supabase migration repair --status applied 20260917000000
npx supabase migration list
```

Expected: as três migrations aparecem com marca nas colunas Local e Remote.

- [ ] **Step 6: Subir o banco local e conferir que ele nasce igual à produção**

```bash
npx supabase start
npx supabase db reset
```

Expected: `supabase start` imprime as URLs locais (API em `http://127.0.0.1:54321`), e `db reset` aplica as três migrations sem erro.

- [ ] **Step 7: Provar que local e produção têm o mesmo schema**

```bash
npx supabase db diff --linked --schema public
```

Expected: saída vazia, ou seja, nenhuma diferença. Se aparecer diferença, não siga: registre o SQL que apareceu e trate como defeito do baseline.

- [ ] **Step 8: Escrever a migration de privilégios padrão**

Create `supabase/migrations/20260923000000_default_privileges_anon.sql`:

```sql
-- Recomendação da revisão final da Fase 0: a Supabase concede privilégios ao papel
-- `anon` automaticamente em tabelas novas do schema public. A Fase 0 revogou o que
-- existia, mas qualquer tabela criada depois nasceria aberta de novo.
-- Isto fecha a porta para o futuro. Não altera dados.

alter default privileges in schema public revoke all on tables from anon;
alter default privileges in schema public revoke all on sequences from anon;
alter default privileges in schema public revoke all on functions from anon;

-- Sequências que já existem também não precisam ficar acessíveis ao anon.
revoke all on all sequences in schema public from anon;
```

- [ ] **Step 9: Aplicar localmente e depois em produção**

```bash
npx supabase db reset
npx supabase db push
```

Expected: `db reset` aplica as quatro migrations; `db push` aplica só a nova em produção e responde `Finished supabase db push`.

- [ ] **Step 10: Adicionar os atalhos no package.json**

Em `package.json`, dentro de `"scripts"`, acrescente:

```json
"db:start": "supabase start",
"db:stop": "supabase stop",
"db:reset": "supabase db reset",
"db:diff": "supabase db diff --linked --schema public",
"db:push": "supabase db push",
"db:types": "supabase gen types typescript --linked > src/types/database.types.ts"
```

- [ ] **Step 11: Commit**

```bash
git add supabase/migrations package.json
git commit -m "feat: banco local com migrations versionadas e privilégios fechados para anon

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: React 19

**Files:**

- Modify: `package.json`, `package-lock.json`, e qualquer arquivo que quebre na compilação por causa de ícone renomeado

**Interfaces:**

- Produces: React 19.3 instalado, pré-requisito do React Router 8 na Task 9.

- [ ] **Step 1: Atualizar React e os ícones**

```bash
npm i react@^19.3.0 react-dom@^19.3.0 lucide-react@^1.47.0
npm i -D @types/react@^19 @types/react-dom@^19
```

- [ ] **Step 2: Rodar o build para descobrir ícones renomeados**

```bash
npm run build
```

Expected: pode falhar com algo como `"Edit2" is not exported by lucide-react`. A biblioteca saiu da 0.263 para a 1.x e alguns nomes mudaram.

- [ ] **Step 3: Corrigir cada ícone que o build apontar**

Para cada nome quebrado, procure o substituto no pacote instalado e troque o import no arquivo indicado:

```bash
node -e "const i=require('lucide-react');console.log(Object.keys(i).filter(n=>/^Edit|^Pencil|^Trash|^Square/.test(n)).join(', '))"
```

Troque apenas o nome do ícone, nunca o tamanho ou as props. Repita o par build/correção até o build passar. Anote no relatório todos os ícones trocados.

- [ ] **Step 4: Verificar**

```bash
npm run lint && npm run test:run && npm run build
```

Expected: tudo passa.

- [ ] **Step 5: Conferir no navegador**

```bash
npm run dev
```

Abra `http://localhost:3000`, faça login e passe por Dashboard, Extrato e Cartões. Confirme que os ícones aparecem e que o console não tem erro novo. Pare o servidor depois.

- [ ] **Step 6: Commit**

```bash
git add package.json package-lock.json src
git commit -m "build: React 19 e lucide-react 1.x

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Vite 8, Vitest 5 e configuração unificada

**Files:**

- Create: `vite.config.ts`
- Delete: `vite.config.js`, `vitest.config.js`
- Modify: `package.json`

**Interfaces:**

- Consumes: React 19 da Task 2.
- Produces: alias `@` apontando para `src` tanto no build quanto nos testes; um único arquivo de configuração.

- [ ] **Step 1: Atualizar as ferramentas de build**

```bash
npm i -D vite@^8.3.0 @vitejs/plugin-react@^6.1.1 vitest@^5.0.1 @vitest/ui@^5.0.1
```

- [ ] **Step 2: Criar o `vite.config.ts` unificado**

Create `vite.config.ts`:

```ts
import path from 'node:path'
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
    },
  },
  server: {
    port: 3000,
  },
  build: {
    rollupOptions: {
      output: {
        // O Vite 8 empacota com Rolldown, que aceita `manualChunks` apenas como
        // função. A forma de objeto, usada até o Vite 7, quebra o build aqui.
        manualChunks(id) {
          if (id.includes('node_modules/recharts')) return 'recharts'
          if (id.includes('node_modules/@supabase')) return 'supabase'
          if (id.includes('node_modules/react-dom') || id.includes('node_modules/react/')) {
            return 'react'
          }
        },
      },
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.js',
    css: true,
    env: {
      VITE_SUPABASE_URL: 'http://localhost:54321',
      VITE_SUPABASE_ANON_KEY: 'test-anon-key',
    },
  },
})
```

- [ ] **Step 3: Remover os arquivos antigos**

```bash
git rm vite.config.js vitest.config.js
```

- [ ] **Step 3b: Tirar o `date-fns`, que não é usado**

```bash
grep -rn "date-fns" src
```

Expected: apenas `src/components/Dashboard/DashboardHome.jsx`, com `import { format } from 'date-fns'` e `import { ptBR } from 'date-fns/locale'`, e nenhum uso de `format` nem de `ptBR` no corpo do arquivo. Confirme isso antes de continuar. Apague as duas linhas de import e desinstale:

```bash
npm uninstall date-fns
```

- [ ] **Step 4: Verificar**

```bash
npm run test:run && npm run build && npm run lint
```

Expected: 26 testes passando, build concluído. Se o Vitest reclamar que não encontra a configuração de teste, confirme que `vite.config.ts` tem o bloco `test` e que o script `test:run` continua `vitest --run`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "build: Vite 8, Vitest 5 e configuração unificada em vite.config.ts

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: TypeScript e tipos do banco

**Files:**

- Create: `tsconfig.json`, `tsconfig.node.json`, `src/vite-env.d.ts`, `src/types/database.types.ts` (gerado)
- Modify: `package.json`

**Interfaces:**

- Produces: `npm run typecheck`; alias `@/*` reconhecido pelo editor; tipo `Database` exportado de `@/types/database.types` para as Tasks 7 e 8.

- [ ] **Step 1: Instalar o TypeScript**

```bash
npm i -D typescript@5.9.3 @types/node
```

- [ ] **Step 2: Criar o `tsconfig.json`**

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "lib": ["ES2023", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "moduleDetection": "force",
    "jsx": "react-jsx",
    "noEmit": true,
    "skipLibCheck": true,
    "allowJs": true,
    "checkJs": false,
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "verbatimModuleSyntax": true,
    "erasableSyntaxOnly": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **Step 3: Criar o `tsconfig.node.json`**

Create `tsconfig.node.json`:

```json
{
  "compilerOptions": {
    "target": "ES2023",
    "lib": ["ES2023"],
    "module": "ESNext",
    "moduleResolution": "bundler",
    "types": ["node"],
    "skipLibCheck": true,
    "strict": true,
    "composite": true
  },
  "include": ["vite.config.ts"]
}
```

Repare que não há `noEmit` aqui. O TypeScript recusa a combinação com `composite` (erro TS6310, "projeto referenciado não pode desabilitar emit"), e o scaffold padrão do Vite também não a usa. O `tsc --noEmit` do script `typecheck` continua suprimindo qualquer saída.

- [ ] **Step 4: Declarar as variáveis de ambiente**

Create `src/vite-env.d.ts`:

```ts
/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_SUPABASE_URL: string
  readonly VITE_SUPABASE_ANON_KEY: string
  readonly VITE_SENTRY_DSN?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
```

- [ ] **Step 5: Gerar os tipos do banco**

```bash
mkdir -p src/types
npm run db:types
head -30 src/types/database.types.ts
```

Expected: o arquivo começa com `export type Json = ...` e traz `export type Database = { public: { Tables: { cartoes_credito: ... } } }`. Se vier vazio, confirme que o `supabase link` aponta para `ctchgljqbardmzajgyjp`.

- [ ] **Step 6: Adicionar o script de checagem**

Em `package.json`, dentro de `"scripts"`:

```json
"typecheck": "tsc --noEmit"
```

- [ ] **Step 7: Verificar**

```bash
npm run typecheck && npm run build && npm run test:run
```

Expected: `typecheck` sem erro (com `checkJs: false`, os `.jsx` atuais não são verificados ainda).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "build: TypeScript 5.9 e tipos gerados do banco

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: ESLint 10, Prettier e ganchos de commit

**Files:**

- Modify: `eslint.config.js`, `package.json`, `.prettierignore`
- Create: `.husky/pre-commit`

**Interfaces:**

- Consumes: TypeScript da Task 4.
- Produces: lint entendendo `.ts`/`.tsx`; formatação e lint automáticos antes de cada commit.

- [ ] **Step 1: Atualizar e instalar**

```bash
npm i -D eslint@^10.11.0 typescript-eslint@^8.70.1 husky@^9.1.7 lint-staged@^17.5.1
```

- [ ] **Step 2: Reescrever o `eslint.config.js`**

Substitua todo o conteúdo de `eslint.config.js` por:

```js
import js from '@eslint/js'
import globals from 'globals'
import tseslint from 'typescript-eslint'
import react from 'eslint-plugin-react'
import reactHooks from 'eslint-plugin-react-hooks'
import jsxA11y from 'eslint-plugin-jsx-a11y'
import prettier from 'eslint-config-prettier'

export default tseslint.config(
  {
    ignores: [
      'dist/**',
      'node_modules/**',
      'coverage/**',
      'supabase/**',
      'docs/**',
      'src/types/database.types.ts',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  react.configs.flat.recommended,
  react.configs.flat['jsx-runtime'],
  jsxA11y.flatConfigs.recommended,
  {
    files: ['**/*.{js,jsx,ts,tsx}'],
    plugins: { 'react-hooks': reactHooks },
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
      globals: { ...globals.browser, ...globals.node },
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    // A versão do React é fixada à mão, não detectada. O `eslint-plugin-react`
    // 7.37.5, que é a última publicada, ainda chama `context.getFilename()`,
    // removido no ESLint 10, e a detecção automática derruba o lint inteiro.
    // Atualize esta string quando o React subir de versão, e volte para
    // 'detect' quando o plugin publicar suporte ao ESLint 10.
    settings: { react: { version: '19.3.0' } },
    rules: {
      'react/prop-types': 'off',
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': [
        'warn',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
      'no-console': ['warn', { allow: ['warn', 'error'] }],
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
      'jsx-a11y/click-events-have-key-events': 'warn',
      'jsx-a11y/no-static-element-interactions': 'warn',
    },
  },
  prettier
)
```

- [ ] **Step 3: Rodar o lint e conferir o número de avisos**

```bash
npm run lint
```

Expected: zero erros. O número de avisos pode subir porque o `@typescript-eslint/no-unused-vars` agora também olha os `.jsx`. Se passar de 50, ajuste o script `lint` em `package.json` para o número real arredondado para cima (por exemplo `--max-warnings 60`) e registre o número no relatório. Não desligue regra para baixar a conta.

- [ ] **Step 4: Ampliar o Prettier para TypeScript**

Em `package.json`, troque o script `format` por:

```json
"format": "prettier --write \"src/**/*.{js,jsx,ts,tsx,css,md}\""
```

Em `.prettierignore`, acrescente uma linha:

```
src/types/database.types.ts
```

- [ ] **Step 5: Instalar o gancho de pré-commit**

```bash
npx husky init
```

Isso cria `.husky/pre-commit`. Substitua o conteúdo do arquivo por:

```sh
npx lint-staged
```

- [ ] **Step 6: Configurar o lint-staged**

Em `package.json`, no primeiro nível do objeto (ao lado de `"scripts"`), acrescente:

```json
"lint-staged": {
  "*.{js,jsx,ts,tsx}": [
    "eslint --fix",
    "prettier --write"
  ],
  "*.{css,md,json}": [
    "prettier --write"
  ]
}
```

- [ ] **Step 7: Provar que o gancho funciona**

```bash
npm run format
git add -A
git commit -m "build: ESLint 10 com TypeScript, Prettier e gancho de pré-commit

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

Expected: antes de concluir o commit aparece a saída do `lint-staged`. Se o commit for recusado por erro de lint, corrija o erro e repita.

- [ ] **Step 8: Verificar**

```bash
npm run lint && npm run typecheck && npm run test:run && npm run build
```

---

### Task 6: Integração contínua no GitHub

**Files:**

- Create: `.github/workflows/ci.yml`

**Interfaces:**

- Produces: verificação automática em todo push e pull request.

- [ ] **Step 1: Criar o fluxo**

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: ['**']
  pull_request:
    branches: [main]

# Cancela execucao superada da mesma referencia. Sem isso, uma branch com PR
# aberto roda o fluxo duas vezes a cada push: uma pelo evento push e outra
# pelo pull_request.
concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

jobs:
  verificar:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7

      - uses: actions/setup-node@v7
        with:
          node-version: 24
          cache: npm

      - name: Instalar dependências
        run: npm ci

      - name: Lint
        run: npm run lint

      - name: Checagem de tipos
        run: npm run typecheck

      - name: Testes
        run: npm run test:run

      - name: Build
        run: npm run build
        env:
          VITE_SUPABASE_URL: https://exemplo.supabase.co
          VITE_SUPABASE_ANON_KEY: chave-de-teste
```

- [ ] **Step 2: Commit e envio**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: lint, tipos, testes e build a cada push e PR

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
git push -u origin fase-1/fundacao
```

- [ ] **Step 3: Conferir a execução**

```bash
curl -s "https://api.github.com/repos/iagosofiatti12/Sofinance/actions/runs?branch=fase-1%2Ffundacao&per_page=3"
```

Expected: a execução aparece como `completed success`. Se o `gh` não estiver autenticado, abra a aba Actions do repositório no navegador e confirme o check verde. Se algum passo falhar, corrija antes de seguir.

---

### Task 7: Regras puras viram `src/domain` em TypeScript

**Files:**

- Move and convert: `src/utils/{currency,dates,vencimentos,resumo,transacaoPayload,validations}.js` → `src/domain/*.ts` (com os testes correspondentes)
- Move and convert: `src/utils/{logger,errorHandler}.js` → `src/lib/*.ts`
- Modify: todos os arquivos que importam esses módulos

**Interfaces:**

- Consumes: `tsconfig.json` da Task 4.
- Produces: `@/domain/currency` exporta `formatCurrency(value: string | number): string` e `parseCurrency(formatted: string): number`; `@/domain/dates` exporta `toISODateLocal(d: Date): string`, `hojeISO(): string`, `parseISODateLocal(iso: string): Date`, `formatarData(iso: string | null | undefined): string`, `formatarMesExtenso(mesRef: string): string`, `formatMesReferencia(date: Date): string`, `mudarMes(mesRef: string, delta: number): string`; `@/domain/vencimentos` exporta `diasAteVencimento(diaVencimento: number | null | undefined, hoje?: Date): number` e `proximosVencimentos<T extends ContaComVencimento>(contas: T[], hoje?: Date, limite?: number): (T & { diasRestantes: number })[]`; `@/domain/resumo` exporta `linhaParaResumo`, `ultimosMeses`, `montarEvolucao`; `@/domain/transacaoPayload` exporta `montarPayloadTransacao`; `@/domain/validations` exporta os schemas Zod e `validateData`; `@/lib/logger` exporta o logger padrão; `@/lib/errorHandler` exporta `getErrorMessage`.

- [ ] **Step 1: Mover os arquivos preservando o histórico**

```bash
mkdir -p src/domain src/lib
git mv src/utils/currency.js src/domain/currency.ts
git mv src/utils/currency.test.js src/domain/currency.test.ts
git mv src/utils/dates.js src/domain/dates.ts
git mv src/utils/dates.test.js src/domain/dates.test.ts
git mv src/utils/vencimentos.js src/domain/vencimentos.ts
git mv src/utils/vencimentos.test.js src/domain/vencimentos.test.ts
git mv src/utils/resumo.js src/domain/resumo.ts
git mv src/utils/resumo.test.js src/domain/resumo.test.ts
git mv src/utils/transacaoPayload.js src/domain/transacaoPayload.ts
git mv src/utils/transacaoPayload.test.js src/domain/transacaoPayload.test.ts
git mv src/utils/validations.js src/domain/validations.ts
git mv src/utils/validations.test.js src/domain/validations.test.ts
git mv src/utils/logger.js src/lib/logger.ts
git mv src/utils/errorHandler.js src/lib/errorHandler.ts
```

- [ ] **Step 2: Rodar os testes e ver o estrago**

```bash
npm run test:run
```

Expected: falhas de importação, porque os testes importam `./currency` e os módulos importam `./dates`. Os caminhos relativos entre os arquivos movidos juntos continuam válidos; quebram os que cruzam pastas (`resumo.ts` importa `./dates`, isso continua certo; `errorHandler.ts` importa `./logger`, também continua certo). O que quebra é quem está fora: componentes e serviços.

- [ ] **Step 3: Atualizar todos os importadores para o alias**

```bash
grep -rln "utils/currency\|utils/dates\|utils/vencimentos\|utils/resumo\|utils/transacaoPayload\|utils/validations\|utils/logger\|utils/errorHandler" src
```

Para cada arquivo listado, troque o caminho relativo pelo alias. Exemplos exatos das trocas:

```
'../../utils/currency'      -> '@/domain/currency'
'../utils/dates'            -> '@/domain/dates'
'../../utils/validations'   -> '@/domain/validations'
'../../utils/transacaoPayload' -> '@/domain/transacaoPayload'
'../../utils/vencimentos'   -> '@/domain/vencimentos'
'../utils/resumo'           -> '@/domain/resumo'
'../utils/logger'           -> '@/lib/logger'
'../../utils/errorHandler'  -> '@/lib/errorHandler'
```

- [ ] **Step 4: Tipar cada módulo de domínio**

Abra cada arquivo `.ts` movido e acrescente os tipos das assinaturas públicas exatamente como listados no bloco Interfaces acima. Em `vencimentos.ts`, declare e exporte o tipo que o `proximosVencimentos` exige:

```ts
export type ContaComVencimento = {
  dia_vencimento: number | null
  ativa: boolean
}
```

Em `resumo.ts`, declare o formato da linha da view:

```ts
export type LinhaResumo = {
  mes_referencia?: string
  total_receitas?: string | number | null
  total_despesas?: string | number | null
  saldo?: string | number | null
}

export type Resumo = { receitas: number; despesas: number; saldo: number }
```

Não mude nenhuma regra nem nenhum teste. Se a tipagem revelar um comportamento estranho, registre no relatório em vez de corrigir.

- [ ] **Step 5: Verificar**

```bash
npm run typecheck && npm run test:run && npm run lint && npm run build
```

Expected: 26 testes passando, zero erro de tipo.

- [ ] **Step 6: Confirmar que a pasta antiga sumiu**

```bash
ls src/utils 2>/dev/null || echo "src/utils removida"
grep -rn "from '.*utils/" src || echo "sem imports antigos"
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: regras puras em src/domain e infraestrutura em src/lib, tipadas

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: Serviços em TypeScript com os tipos do banco

**Files:**

- Move and convert: `src/services/supabaseClient.js` → `src/lib/supabase.ts`
- Convert: `src/services/{authService,transacoesService,cartoesService,contasService,financiamentosService,metasService}.js` → `.ts`
- Modify: importadores

**Interfaces:**

- Consumes: `Database` de `@/types/database.types` (Task 4); `@/lib/logger` (Task 7).
- Produces: `@/lib/supabase` exporta `supabase` (cliente tipado) e `getUserId(): Promise<string>`; os serviços mantêm exatamente os mesmos nomes de função de hoje, agora tipados. `contasService` exporta `getContasFixas(): Promise<ContaFixa[]>`, `addContaFixa(conta: ContaFixaInput): Promise<ContaFixa>`, `updateContaFixa(id: string, updates: Partial<ContaFixaInput>): Promise<ContaFixa>`, `deleteContaFixa(id: string): Promise<void>`, e os tipos `ContaFixa = Database['public']['Tables']['contas_fixas']['Row']` e `ContaFixaInput = Omit<Database['public']['Tables']['contas_fixas']['Insert'], 'user_id'>` (o servico obtem o user_id da sessao e sobrescreve; deixa-lo no tipo faria o formulario da Task 12 exigir um campo que o chamador nunca deve informar).

- [ ] **Step 1: Mover e tipar o cliente**

```bash
git mv src/services/supabaseClient.js src/lib/supabase.ts
```

Substitua o conteúdo de `src/lib/supabase.ts` por:

```ts
import { createClient } from '@supabase/supabase-js'
import type { Database } from '@/types/database.types'

const supabaseUrl = import.meta.env.VITE_SUPABASE_URL
const supabaseAnonKey = import.meta.env.VITE_SUPABASE_ANON_KEY

if (!supabaseUrl || !supabaseAnonKey) {
  throw new Error('Missing Supabase environment variables. Please check your .env file.')
}

export const supabase = createClient<Database>(supabaseUrl, supabaseAnonKey, {
  auth: {
    persistSession: true,
    autoRefreshToken: true,
    detectSessionInUrl: true,
    storage: window.localStorage,
  },
})

export const getUserId = async (): Promise<string> => {
  const {
    data: { user },
    error,
  } = await supabase.auth.getUser()

  if (error || !user) {
    throw new Error('Usuário não autenticado')
  }

  return user.id
}
```

- [ ] **Step 2: Atualizar quem importava o cliente**

```bash
grep -rln "services/supabaseClient" src
```

Troque cada ocorrência por `'@/lib/supabase'`.

- [ ] **Step 3: Renomear os serviços para TypeScript**

```bash
for f in authService transacoesService cartoesService contasService financiamentosService metasService; do
  git mv "src/services/$f.js" "src/services/$f.ts"
done
```

- [ ] **Step 4: Rodar a checagem de tipos e corrigir arquivo por arquivo**

```bash
npm run typecheck
```

Para cada erro, acrescente o tipo que falta. Regras a seguir:

- Linhas vindas do banco usam `Database['public']['Tables']['<tabela>']['Row']`.
- Dados enviados usam `['Insert']` ou `['Update']`.
- Nada de `any`. Se precisar de uma saída, use `unknown` e estreite com verificação.
- Não mude nenhuma consulta nem nenhuma regra. Se o TypeScript apontar um erro real de lógica, registre no relatório e não corrija aqui.

Comece por `contasService.ts`, que é o serviço da tela piloto, e deixe-o exatamente com as assinaturas do bloco Interfaces.

- [ ] **Step 5: Verificar**

```bash
npm run typecheck && npm run lint && npm run test:run && npm run build
```

- [ ] **Step 6: Conferir no navegador**

```bash
npm run dev
```

Faça login, abra Dashboard, Extrato, Contas Fixas, Cartões e Metas. Nenhum erro novo no console. Pare o servidor.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: serviços e cliente Supabase em TypeScript com tipos do banco

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 9: React Router com URLs de verdade

**Files:**

- Create: `src/app/router.tsx`, `src/app/RequireAuth.tsx`, `src/app/AppLayout.tsx`
- Modify: `src/main.jsx` → `src/main.tsx`, `src/components/Layout/Sidebar.jsx`, `src/components/Auth/AuthPage.jsx`, `src/components/Auth/Login.jsx`, `src/components/Auth/SignUp.jsx`, `src/components/Auth/ForgotPassword.jsx`, `src/components/Auth/ResetPassword.jsx`, `src/components/Settings/Settings.jsx`
- Delete: `src/App.jsx`, `src/App.css` passa a ser importado pelo `AppLayout`

**Interfaces:**

- Consumes: `useAuth()` de `@/contexts/AuthContext` com `{ isAuthenticated, loading, recoveryMode, clearRecovery, user, signOut }`.
- Produces: rotas `/login`, `/cadastro`, `/recuperar-senha`, `/reset-password`, `/` (dashboard), `/lancamentos`, `/contas`, `/cartoes`, `/dividas/imovel`, `/dividas/carro`, `/metas`, `/configuracoes`.

**Contexto:** hoje `src/App.jsx` troca de tela com `useState('activeSection')` e a `Sidebar` recebe `activeSection` e `setActiveSection` como props. Depois desta task a Sidebar usa `NavLink` e não recebe mais props.

- [ ] **Step 1: Instalar o roteador**

```bash
npm i react-router@^8.4.0
```

- [ ] **Step 2: Criar a guarda de autenticação**

Create `src/app/RequireAuth.tsx`:

```tsx
import { Navigate, Outlet, useLocation } from 'react-router'
import { useAuth } from '@/contexts/AuthContext'
import Spinner from '@/components/UI/Spinner'

export default function RequireAuth() {
  const { isAuthenticated, loading, recoveryMode } = useAuth()
  const location = useLocation()

  if (loading) {
    return (
      <div className="app-loading">
        <Spinner label="Carregando..." size={56} />
      </div>
    )
  }

  // O link do e-mail de recuperação já cria uma sessão válida. Mandar para a
  // troca de senha antes de liberar o resto do app.
  if (recoveryMode && location.pathname !== '/reset-password') {
    return <Navigate to="/reset-password" replace />
  }

  if (!isAuthenticated) {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />
  }

  return <Outlet />
}
```

- [ ] **Step 3: Criar a moldura do app**

Create `src/app/AppLayout.tsx`:

```tsx
import { Suspense } from 'react'
import { Outlet } from 'react-router'
import { Toaster } from 'react-hot-toast'
import DarkModeToggle from '@/components/Layout/DarkModeToggle'
import Sidebar from '@/components/Layout/Sidebar'
import Spinner from '@/components/UI/Spinner'
import '@/App.css'

export default function AppLayout() {
  return (
    <div className="app">
      <Toaster
        position="top-right"
        toastOptions={{
          duration: 3000,
          style: {
            background: 'var(--card-bg)',
            color: 'var(--text-primary)',
            border: '1.5px solid var(--glass-border)',
            backdropFilter: 'blur(10px)',
            fontSize: '13px',
            fontWeight: '600',
          },
          success: {
            iconTheme: { primary: 'var(--accent-green)', secondary: 'var(--card-bg)' },
          },
          error: {
            iconTheme: { primary: 'var(--accent-red)', secondary: 'var(--card-bg)' },
          },
        }}
      />

      <div className="dark-mode-float">
        <DarkModeToggle />
      </div>

      <div className="app-container">
        <Sidebar />
        <main className="main-content">
          <div className="content-wrapper">
            <Suspense fallback={<Spinner />}>
              <Outlet />
            </Suspense>
          </div>
        </main>
      </div>
    </div>
  )
}
```

- [ ] **Step 4: Criar o roteador**

Create `src/app/router.tsx`:

```tsx
import { lazy } from 'react'
import { createBrowserRouter } from 'react-router'
import RequireAuth from './RequireAuth'
import AppLayout from './AppLayout'
import Login from '@/components/Auth/Login'
import SignUp from '@/components/Auth/SignUp'
import ForgotPassword from '@/components/Auth/ForgotPassword'
import ResetPassword from '@/components/Auth/ResetPassword'

const DashboardHome = lazy(() => import('@/components/Dashboard/DashboardHome'))
const ExtratoMensal = lazy(() => import('@/components/Extrato/ExtratoMensal'))
const ContasFixasList = lazy(() => import('@/components/ContasFixas/ContasFixasList'))
const CartoesList = lazy(() => import('@/components/Cartoes/CartoesList'))
const FinanciamentoImovel = lazy(() => import('@/components/Financiamentos/FinanciamentoImovel'))
const FinanciamentoCarro = lazy(() => import('@/components/Financiamentos/FinanciamentoCarro'))
const MetasList = lazy(() => import('@/components/Metas/MetasList'))
const Settings = lazy(() => import('@/components/Settings/Settings'))

export const router = createBrowserRouter([
  { path: '/login', Component: Login },
  { path: '/cadastro', Component: SignUp },
  { path: '/recuperar-senha', Component: ForgotPassword },
  { path: '/reset-password', Component: ResetPassword },
  {
    Component: RequireAuth,
    children: [
      {
        path: '/',
        Component: AppLayout,
        children: [
          { index: true, Component: DashboardHome },
          { path: 'lancamentos', Component: ExtratoMensal },
          { path: 'contas', Component: ContasFixasList },
          { path: 'cartoes', Component: CartoesList },
          { path: 'dividas/imovel', Component: FinanciamentoImovel },
          { path: 'dividas/carro', Component: FinanciamentoCarro },
          { path: 'metas', Component: MetasList },
          { path: 'configuracoes', Component: Settings },
        ],
      },
    ],
  },
])
```

- [ ] **Step 5: Trocar o ponto de entrada**

```bash
git mv src/main.jsx src/main.tsx
```

Substitua o conteúdo de `src/main.tsx` por:

```tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import * as Sentry from '@sentry/react'
import { RouterProvider } from 'react-router'
import { router } from '@/app/router'
import ErrorBoundary from '@/components/ErrorBoundary'
import { AuthProvider } from '@/contexts/AuthContext'
import '@/styles/index.css'

if (import.meta.env.VITE_SENTRY_DSN) {
  Sentry.init({
    dsn: import.meta.env.VITE_SENTRY_DSN,
    environment: import.meta.env.MODE,
    sendDefaultPii: false,
    tracesSampleRate: 0.1,
  })
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <ErrorBoundary>
      <AuthProvider>
        <RouterProvider router={router} />
      </AuthProvider>
    </ErrorBoundary>
  </React.StrictMode>
)
```

Atualize `index.html` para apontar para o novo arquivo: troque `src="/src/main.jsx"` por `src="/src/main.tsx"`.

- [ ] **Step 6: Apagar o App antigo**

```bash
git rm src/App.jsx
```

- [ ] **Step 7: Converter a Sidebar para links**

Em `src/components/Layout/Sidebar.jsx`, remova as props `activeSection` e `setActiveSection`, troque a lista de itens e os botões por `NavLink`:

```jsx
import { NavLink, useNavigate } from 'react-router'

const menuItems = [
  { to: '/', label: 'Dashboard', icon: LayoutDashboard, end: true },
  { to: '/lancamentos', label: 'Extrato Mensal', icon: FileText },
  { to: '/contas', label: 'Contas Fixas', icon: Receipt },
  { to: '/cartoes', label: 'Cartões', icon: CreditCard },
  { to: '/dividas/imovel', label: 'Financ. Imóvel', icon: Home },
  { to: '/dividas/carro', label: 'Financ. Carro', icon: Car },
  { to: '/metas', label: 'Metas', icon: Target },
]
```

Cada item vira:

```jsx
<NavLink
  key={item.to}
  to={item.to}
  end={item.end}
  className={({ isActive }) => `sidebar-item ${isActive ? 'active' : ''}`}
  aria-label={`Navegar para ${item.label}`}
>
  <Icon size={20} aria-hidden="true" />
  <span>{item.label}</span>
</NavLink>
```

O botão "Perfil" vira `<NavLink to="/configuracoes" className="sidebar-item sidebar-action">`. O clique no logo vira `<NavLink to="/">`. O botão "Sair" continua chamando `signOut()`, e depois disso a guarda leva para `/login` sozinha.

- [ ] **Step 8: Ligar as telas de autenticação pelas rotas**

Em `src/components/Auth/Login.jsx`: remova as props `onToggleMode` e `onForgotPassword`; use `useNavigate()` e troque os botões do rodapé por navegação para `/cadastro` e `/recuperar-senha`. Após o login bem-sucedido, chame `navigate('/', { replace: true })`.

Em `src/components/Auth/SignUp.jsx`: troque `onToggleMode()` por `navigate('/login')`, inclusive no `setTimeout` da tela de sucesso.

Em `src/components/Auth/ForgotPassword.jsx`: troque a prop `onBack` por `navigate('/login')`.

Em `src/components/Auth/ResetPassword.jsx`: depois de salvar a senha, troque `window.history.replaceState({}, '', '/')` por `navigate('/', { replace: true })`, mantendo a chamada a `clearRecovery()` antes.

```bash
git rm src/components/Auth/AuthPage.jsx
```

- [ ] **Step 9: Ajustar o link interno das Configurações**

Em `src/components/Settings/Settings.jsx`, nada muda na lógica. Confirme apenas que nenhum texto menciona a navegação antiga.

- [ ] **Step 10: Verificar**

```bash
npm run typecheck && npm run lint && npm run test:run && npm run build
```

- [ ] **Step 11: Conferir no navegador**

```bash
npm run dev
```

Confirme, um a um:

1. Sem sessão, abrir `http://localhost:3000/contas` manda para `/login`.
2. Depois do login, a URL vira `/` e o dashboard aparece.
3. Clicar em cada item do menu muda a URL e destaca o item certo.
4. Atualizar a página em `/cartoes` mantém a tela de cartões.
5. O botão voltar do navegador funciona.
6. `/recuperar-senha` abre a tela de recuperação.

Pare o servidor.

- [ ] **Step 12: Commit**

```bash
git add -A
git commit -m "feat: React Router com URLs reais, guarda de autenticação e layout compartilhado

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 10: TanStack Query

**Files:**

- Create: `src/lib/queryClient.ts`, `src/app/providers.tsx`
- Modify: `src/main.tsx`

**Interfaces:**

- Consumes: roteador da Task 9.
- Produces: `queryClient` exportado de `@/lib/queryClient`; `<Providers>` de `@/app/providers` embrulhando autenticação e cache; `useQuery`/`useMutation` disponíveis para a Task 12.

- [ ] **Step 1: Instalar**

```bash
npm i @tanstack/react-query@^5.103.2
npm i -D @tanstack/react-query-devtools@^5.103.2
```

- [ ] **Step 2: Criar o cliente de cache**

Create `src/lib/queryClient.ts`:

```ts
import { QueryClient } from '@tanstack/react-query'

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Dados financeiros mudam pouco durante uma sessão; 30s evita refazer
      // a mesma consulta ao trocar de tela e voltar.
      staleTime: 30_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
})
```

- [ ] **Step 3: Reunir os providers**

Create `src/app/providers.tsx`:

```tsx
import type { ReactNode } from 'react'
import { QueryClientProvider } from '@tanstack/react-query'
import { ReactQueryDevtools } from '@tanstack/react-query-devtools'
import { AuthProvider } from '@/contexts/AuthContext'
import { queryClient } from '@/lib/queryClient'

export default function Providers({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      <AuthProvider>{children}</AuthProvider>
      {import.meta.env.DEV && <ReactQueryDevtools initialIsOpen={false} />}
    </QueryClientProvider>
  )
}
```

- [ ] **Step 4: Usar no ponto de entrada**

Em `src/main.tsx`, troque o par `<AuthProvider>...</AuthProvider>` por `<Providers>...</Providers>`, importando de `@/app/providers` e removendo o import direto de `AuthProvider`.

- [ ] **Step 5: Verificar**

```bash
npm run typecheck && npm run lint && npm run test:run && npm run build
```

- [ ] **Step 6: Conferir no navegador**

```bash
npm run dev
```

O app continua funcionando igual e, em desenvolvimento, aparece o botão flutuante das ferramentas do React Query. Pare o servidor.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: TanStack Query com cliente único e providers reunidos

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 11: Tailwind v4 e shadcn/ui

**Files:**

- Modify: `vite.config.ts`, `src/styles/index.css`, `package.json`
- Create: `components.json`, `src/lib/utils.ts`, `src/components/ui/*`

**Interfaces:**

- Produces: classes do Tailwind disponíveis em qualquer componente; `cn()` exportado de `@/lib/utils`; componentes `Button`, `Card`, `Dialog`, `Input`, `Label`, `Table` e `Badge` em `@/components/ui/`.

**Contexto:** o CSS atual (`src/styles/index.css` e os `.css` por tela) continua valendo. Tailwind entra ao lado, não no lugar. A troca acontece tela a tela, começando pela Task 12.

- [ ] **Step 1: Instalar o Tailwind**

```bash
npm i -D tailwindcss@^4.3.3 @tailwindcss/vite@^4.3.3
```

- [ ] **Step 2: Ligar o plugin no Vite**

Em `vite.config.ts`, importe e acrescente o plugin:

```ts
import tailwindcss from '@tailwindcss/vite'
```

e troque a linha dos plugins por:

```ts
  plugins: [react(), tailwindcss()],
```

- [ ] **Step 3: Importar o Tailwind e declarar os tokens da marca**

No topo de `src/styles/index.css`, antes de qualquer outra regra, acrescente:

```css
@import 'tailwindcss';

@theme {
  --color-brand: #2563eb;
  --color-brand-light: #3b82f6;
  --color-brand-dark: #1d4ed8;
  --color-receita: #10b981;
  --color-despesa: #ef4444;
  --color-alerta: #f59e0b;
  --radius-card: 20px;
}
```

Não remova nada do CSS existente nesta task.

- [ ] **Step 4: Conferir que o Tailwind funciona**

```bash
npm run build
```

Expected: build conclui. O CSS gerado cresce, o que é esperado.

- [ ] **Step 5: Inicializar o shadcn com base Radix**

```bash
npx shadcn@latest init -y -b radix -p nova --no-monorepo
```

Expected: cria `components.json`, cria `src/lib/utils.ts` com a função `cn` e acrescenta as variáveis de tema do shadcn ao CSS.

- [ ] **Step 6: Conferir o `components.json`**

Abra `components.json` e confirme que os aliases apontam para o nosso alias `@`:

```json
{
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  }
}
```

Se o arquivo trouxer outro prefixo, corrija para `@`.

- [ ] **Step 7: Instalar os componentes que a tela piloto vai usar**

```bash
npx shadcn@latest add button card dialog input label table badge select checkbox
```

Expected: os arquivos aparecem em `src/components/ui/`.

- [ ] **Step 8: Garantir que o CSS antigo não foi sobrescrito**

```bash
git diff --stat src/styles/index.css
grep -c "glass-card" src/styles/index.css
```

Expected: o arquivo cresceu, e `glass-card` continua presente. Se o shadcn tiver apagado o CSS anterior, recupere com `git checkout -p` e reaplique só as adições.

- [ ] **Step 9: Verificar**

```bash
npm run typecheck && npm run lint && npm run test:run && npm run build
```

Expected: pode ser necessário acrescentar `src/components/ui/**` à lista `ignores` do ESLint se os componentes gerados trouxerem avisos. Registre no relatório se fizer isso.

- [ ] **Step 10: Conferir no navegador que nada mudou de aparência**

```bash
npm run dev
```

Passe pelas telas. O visual deve estar idêntico ao de antes. Pare o servidor.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat: Tailwind v4 e shadcn/ui com base Radix, convivendo com o CSS atual

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 12: Tela piloto — Contas Fixas com Query, TypeScript e shadcn

**Files:**

- Create: `src/features/contas-fixas/api.ts`, `src/features/contas-fixas/queries.ts`, `src/features/contas-fixas/ContasFixasPage.tsx`, `src/features/contas-fixas/queries.test.ts`, `src/components/ui/currency-input.tsx`
- Modify: `src/app/router.tsx`, `package.json`
- Delete: `src/components/ContasFixas/ContasFixasList.jsx`, `src/components/ContasFixas/ContasFixas.css`

**Interfaces:**

- Consumes: `contasService` da Task 8, `queryClient` da Task 10, componentes de `@/components/ui` da Task 11, `diasAteVencimento` de `@/domain/vencimentos`, `formatCurrency` de `@/domain/currency`, `contaFixaSchema` de `@/domain/validations`, `CATEGORIAS_CONTAS` de `@/config/constants`.
- Produces: `contasFixasKeys`, `useContasFixas()`, `useSalvarContaFixa()`, `useExcluirContaFixa()`, `useAlternarContaFixa()`; `<CurrencyInput value={number} onChange={(n: number) => void} />` em `@/components/ui/currency-input`; rota `/contas` renderizando `ContasFixasPage`.

**Objetivo desta task:** provar o padrão que as Fases 2 e 3 vão repetir. Nenhum comportamento novo para o usuário.

- [ ] **Step 1: Criar a camada de acesso da feature**

Create `src/features/contas-fixas/api.ts`:

```ts
import {
  getContasFixas,
  addContaFixa,
  updateContaFixa,
  deleteContaFixa,
} from '@/services/contasService'
import type { ContaFixa, ContaFixaInput } from '@/services/contasService'

export type { ContaFixa, ContaFixaInput }

export const listar = getContasFixas
export const criar = addContaFixa
export const atualizar = updateContaFixa
export const excluir = deleteContaFixa
```

- [ ] **Step 2: Escrever o teste dos hooks (falhando)**

Create `src/features/contas-fixas/queries.test.ts`:

```ts
import { describe, it, expect } from 'vitest'
import { contasFixasKeys } from './queries'

describe('contasFixasKeys', () => {
  it('tem uma chave estável para a lista', () => {
    expect(contasFixasKeys.all).toEqual(['contas-fixas'])
  })

  it('deriva a chave de uma conta específica da chave da lista', () => {
    expect(contasFixasKeys.detalhe('abc')).toEqual(['contas-fixas', 'abc'])
  })
})
```

- [ ] **Step 3: Rodar e ver falhar**

```bash
npm run test:run -- src/features/contas-fixas/queries.test.ts
```

Expected: FAIL, módulo `./queries` não encontrado.

- [ ] **Step 4: Implementar os hooks**

Create `src/features/contas-fixas/queries.ts`:

```ts
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { listar, criar, atualizar, excluir } from './api'
import type { ContaFixa, ContaFixaInput } from './api'

export const contasFixasKeys = {
  all: ['contas-fixas'] as const,
  detalhe: (id: string) => ['contas-fixas', id] as const,
}

export function useContasFixas() {
  return useQuery({
    queryKey: contasFixasKeys.all,
    queryFn: listar,
  })
}

export function useSalvarContaFixa() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: async (entrada: { id?: string; dados: ContaFixaInput }) =>
      entrada.id ? atualizar(entrada.id, entrada.dados) : criar(entrada.dados),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: contasFixasKeys.all })
    },
  })
}

export function useExcluirContaFixa() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => excluir(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: contasFixasKeys.all })
    },
  })
}

export function useAlternarContaFixa() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (conta: ContaFixa) => atualizar(conta.id, { ativa: !conta.ativa }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: contasFixasKeys.all })
    },
  })
}
```

- [ ] **Step 5: Rodar e ver passar**

```bash
npm run test:run -- src/features/contas-fixas/queries.test.ts
```

Expected: 2 testes passando.

- [ ] **Step 5b: Instalar o formulário**

```bash
npm i react-hook-form@^7.88.0 @hookform/resolvers@^5.9.1
```

- [ ] **Step 5c: Criar o campo de moeda reutilizável**

Create `src/components/ui/currency-input.tsx`:

```tsx
import { Input } from '@/components/ui/input'
import { formatCurrency } from '@/domain/currency'

type CurrencyInputProps = {
  id?: string
  value: number
  onChange: (valor: number) => void
  onBlur?: () => void
  disabled?: boolean
  'aria-invalid'?: boolean
}

// Guarda o valor como number em reais e mostra formatado. O arredondamento
// antes de formatar evita que 85.9 * 100 vire 8590.000000000001.
export function CurrencyInput({ value, onChange, ...props }: CurrencyInputProps) {
  return (
    <Input
      {...props}
      type="text"
      inputMode="numeric"
      placeholder="R$ 0,00"
      value={value ? formatCurrency(Math.round(value * 100)) : ''}
      onChange={event => {
        const digitos = event.target.value.replace(/\D/g, '')
        onChange(Number(digitos) / 100)
      }}
    />
  )
}
```

- [ ] **Step 6: Escrever a página**

A tela entrega exatamente o mesmo comportamento de `ContasFixasList.jsx`: total mensal das contas ativas no cabeçalho, uma grade de cartões com nome, valor, vencimento, categoria e selo de dias restantes, botões de editar, excluir e ativar/desativar, estado vazio e um diálogo de cadastro e edição. O que muda é a mecânica.

Create `src/features/contas-fixas/ContasFixasPage.tsx`:

```tsx
import { useState } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import type { z } from 'zod'
import { AlertTriangle, Calendar, Check, DollarSign, Pencil, Plus, Trash2, X } from 'lucide-react'
import toast from 'react-hot-toast'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { CurrencyInput } from '@/components/ui/currency-input'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import EmptyState from '@/components/EmptyState'
import LoadingSkeleton from '@/components/LoadingSkeleton'
import { CATEGORIAS_CONTAS } from '@/config/constants'
import { formatCurrency } from '@/domain/currency'
import { contaFixaSchema } from '@/domain/validations'
import { diasAteVencimento } from '@/domain/vencimentos'
import { getErrorMessage } from '@/lib/errorHandler'
import type { ContaFixa } from './api'
import {
  useAlternarContaFixa,
  useContasFixas,
  useExcluirContaFixa,
  useSalvarContaFixa,
} from './queries'

type FormValues = z.infer<typeof contaFixaSchema>

const VALORES_INICIAIS: FormValues = {
  nome: '',
  valor: 0,
  dia_vencimento: 1,
  categoria: 'Moradia',
  ativa: true,
}

function corDoSelo(dias: number): 'default' | 'secondary' | 'destructive' {
  if (dias <= 3) return 'destructive'
  if (dias <= 7) return 'secondary'
  return 'default'
}

function textoDoSelo(dias: number): string {
  if (dias === 0) return 'Vence hoje'
  return `${dias} dias restantes`
}

export default function ContasFixasPage() {
  const { data: contas = [], isPending, error, refetch } = useContasFixas()
  const salvar = useSalvarContaFixa()
  const excluir = useExcluirContaFixa()
  const alternar = useAlternarContaFixa()

  const [dialogoAberto, setDialogoAberto] = useState(false)
  const [editando, setEditando] = useState<ContaFixa | null>(null)

  const form = useForm<FormValues>({
    resolver: zodResolver(contaFixaSchema),
    defaultValues: VALORES_INICIAIS,
  })

  const abrirDialogo = (conta: ContaFixa | null) => {
    setEditando(conta)
    form.reset(
      conta
        ? {
            nome: conta.nome,
            valor: Number(conta.valor),
            dia_vencimento: conta.dia_vencimento,
            categoria: conta.categoria,
            ativa: conta.ativa ?? true,
          }
        : VALORES_INICIAIS
    )
    setDialogoAberto(true)
  }

  const aoEnviar = form.handleSubmit(async dados => {
    try {
      await salvar.mutateAsync({ id: editando?.id, dados })
      toast.success(editando ? 'Conta atualizada!' : 'Conta adicionada!')
      setDialogoAberto(false)
    } catch (erro) {
      toast.error(getErrorMessage(erro))
    }
  })

  const aoExcluir = async (conta: ContaFixa) => {
    if (!window.confirm(`Excluir a conta "${conta.nome}"?`)) return
    try {
      await excluir.mutateAsync(conta.id)
      toast.success('Conta excluída!')
    } catch (erro) {
      toast.error(getErrorMessage(erro))
    }
  }

  const aoAlternar = async (conta: ContaFixa) => {
    try {
      await alternar.mutateAsync(conta)
      toast.success(conta.ativa ? 'Conta desativada.' : 'Conta ativada.')
    } catch (erro) {
      toast.error(getErrorMessage(erro))
    }
  }

  const totalMensal = contas
    .filter(conta => conta.ativa)
    .reduce((soma, conta) => soma + Number(conta.valor), 0)

  if (error) {
    return (
      <EmptyState
        icon={AlertTriangle}
        title="Não foi possível carregar suas contas"
        message={getErrorMessage(error)}
        actionLabel="Tentar de novo"
        onAction={() => refetch()}
      />
    )
  }

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-wrap items-center justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold">Contas Fixas</h2>
          <p className="text-sm text-[var(--text-secondary)]">
            Total mensal: <strong>{formatCurrency(Math.round(totalMensal * 100))}</strong>
          </p>
        </div>
        <Button onClick={() => abrirDialogo(null)}>
          <Plus size={18} />
          Nova Conta
        </Button>
      </header>

      {isPending ? (
        <LoadingSkeleton type="card" count={3} />
      ) : contas.length === 0 ? (
        <EmptyState
          icon={Calendar}
          title="Nenhuma Conta Cadastrada"
          message="Adicione suas contas fixas mensais para melhor controle financeiro"
          actionLabel="Adicionar Primeira Conta"
          onAction={() => abrirDialogo(null)}
        />
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {contas.map(conta => {
            const dias = diasAteVencimento(conta.dia_vencimento)
            return (
              <Card key={conta.id} className={conta.ativa ? '' : 'opacity-60'}>
                <CardHeader className="flex flex-row items-start justify-between gap-2">
                  <div className="flex flex-col gap-1">
                    <h3 className="font-semibold">{conta.nome}</h3>
                    <Badge variant={conta.ativa ? 'default' : 'destructive'}>
                      {conta.ativa ? 'Ativa' : 'Inativa'}
                    </Badge>
                  </div>
                  <div className="flex gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => abrirDialogo(conta)}
                      aria-label={`Editar conta ${conta.nome}`}
                    >
                      <Pencil size={16} />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => aoExcluir(conta)}
                      aria-label={`Excluir conta ${conta.nome}`}
                    >
                      <Trash2 size={16} />
                    </Button>
                  </div>
                </CardHeader>

                <CardContent className="flex flex-col gap-2 text-sm">
                  <span className="flex items-center gap-2 text-lg font-bold">
                    <DollarSign size={18} />
                    {formatCurrency(Math.round(Number(conta.valor) * 100))}
                  </span>
                  <span className="flex items-center gap-2">
                    <Calendar size={16} />
                    Vencimento: dia {conta.dia_vencimento}
                  </span>
                  <Badge variant="secondary">{conta.categoria}</Badge>
                  {conta.ativa && <Badge variant={corDoSelo(dias)}>{textoDoSelo(dias)}</Badge>}
                </CardContent>

                <CardFooter>
                  <Button
                    variant={conta.ativa ? 'secondary' : 'default'}
                    className="w-full"
                    onClick={() => aoAlternar(conta)}
                    disabled={alternar.isPending}
                  >
                    {conta.ativa ? <X size={16} /> : <Check size={16} />}
                    {conta.ativa ? 'Desativar' : 'Ativar'}
                  </Button>
                </CardFooter>
              </Card>
            )
          })}
        </div>
      )}

      <Dialog open={dialogoAberto} onOpenChange={setDialogoAberto}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{editando ? 'Editar Conta' : 'Nova Conta Fixa'}</DialogTitle>
          </DialogHeader>

          <form onSubmit={aoEnviar} className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="conta-nome">Nome da Conta</Label>
              <Input
                id="conta-nome"
                placeholder="Ex: Aluguel, Luz, Internet..."
                aria-invalid={Boolean(form.formState.errors.nome)}
                {...form.register('nome')}
              />
              {form.formState.errors.nome && (
                <span className="text-sm text-[var(--accent-red)]">
                  {form.formState.errors.nome.message}
                </span>
              )}
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="conta-valor">Valor</Label>
              <Controller
                control={form.control}
                name="valor"
                render={({ field }) => (
                  <CurrencyInput
                    id="conta-valor"
                    value={field.value}
                    onChange={field.onChange}
                    onBlur={field.onBlur}
                    aria-invalid={Boolean(form.formState.errors.valor)}
                  />
                )}
              />
              {form.formState.errors.valor && (
                <span className="text-sm text-[var(--accent-red)]">
                  {form.formState.errors.valor.message}
                </span>
              )}
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="conta-dia">Dia do Vencimento</Label>
              <Input
                id="conta-dia"
                type="number"
                min={1}
                max={31}
                aria-invalid={Boolean(form.formState.errors.dia_vencimento)}
                {...form.register('dia_vencimento', { valueAsNumber: true })}
              />
              {form.formState.errors.dia_vencimento && (
                <span className="text-sm text-[var(--accent-red)]">
                  {form.formState.errors.dia_vencimento.message}
                </span>
              )}
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="conta-categoria">Categoria</Label>
              <Controller
                control={form.control}
                name="categoria"
                render={({ field }) => (
                  <Select value={field.value} onValueChange={field.onChange}>
                    <SelectTrigger id="conta-categoria">
                      <SelectValue placeholder="Selecione..." />
                    </SelectTrigger>
                    <SelectContent>
                      {CATEGORIAS_CONTAS.map(categoria => (
                        <SelectItem key={categoria} value={categoria}>
                          {categoria}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                )}
              />
            </div>

            <div className="flex items-center gap-2">
              <Controller
                control={form.control}
                name="ativa"
                render={({ field }) => (
                  <Checkbox
                    id="conta-ativa"
                    checked={field.value}
                    onCheckedChange={field.onChange}
                  />
                )}
              />
              <Label htmlFor="conta-ativa">Conta ativa</Label>
            </div>

            <DialogFooter>
              <Button type="button" variant="secondary" onClick={() => setDialogoAberto(false)}>
                Cancelar
              </Button>
              <Button type="submit" disabled={salvar.isPending}>
                {salvar.isPending
                  ? 'Salvando...'
                  : editando
                    ? 'Salvar Alterações'
                    : 'Adicionar Conta'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  )
}
```

Observação para quem implementa: o `contaFixaSchema` declara `ativa` como opcional. Se o `zodResolver` reclamar do tipo de `FormValues`, ajuste o schema em `@/domain/validations.ts` para `ativa: z.boolean().default(true)` e rode os testes de validação para confirmar que nada mais quebrou.

- [ ] **Step 7: Apontar a rota para a página nova**

Em `src/app/router.tsx`, troque a importação preguiçosa de `ContasFixasList` por:

```tsx
const ContasFixasPage = lazy(() => import('@/features/contas-fixas/ContasFixasPage'))
```

e o item da rota por `{ path: 'contas', Component: ContasFixasPage }`.

- [ ] **Step 8: Remover a tela antiga**

```bash
git rm src/components/ContasFixas/ContasFixasList.jsx src/components/ContasFixas/ContasFixas.css
grep -rn "ContasFixasList\|ContasFixas.css" src || echo "sem referências antigas"
```

- [ ] **Step 9: Verificar**

```bash
npm run typecheck && npm run lint && npm run test:run && npm run build
```

- [ ] **Step 10: Conferir no navegador contra o comportamento antigo**

```bash
npm run dev
```

Em `/contas`, confirme: a lista carrega; criar uma conta faz a lista se atualizar sozinha; editar mantém os valores no formulário; excluir pede confirmação e some da lista; ativar e desativar muda o selo; o total mensal no cabeçalho bate com a soma das ativas; `Escape` fecha o diálogo; o foco fica preso dentro dele. Apague as contas de teste ao final. Pare o servidor.

- [ ] **Step 11: Commit**

```bash
git add -A
git commit -m "feat: Contas Fixas como tela piloto com TanStack Query, TypeScript e shadcn

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 13: Memória do projeto, revisão e integração

**Files:**

- Create: `CLAUDE.md`, `.claude/skills/sofinance-domain/SKILL.md`
- Modify: `README.md`, `docs/superpowers/specs/2026-09-17-sofinance-revamp-design.md`, `.claude/agents/sofinance-senior-engineer.md`

- [ ] **Step 1: Escrever o `CLAUDE.md`**

Create `CLAUDE.md`:

```markdown
# Sofinance

App brasileiro de finanças pessoais. React 19 + Vite 8 + TypeScript na frente, Supabase (Postgres, Auth, Edge Functions) atrás, Vercel na hospedagem. Público: pessoas físicas no Brasil. Idioma da interface: português do Brasil.

## Comandos

| Comando                        | O que faz                                           |
| ------------------------------ | --------------------------------------------------- |
| `npm run dev`                  | Sobe o app em http://localhost:3000                 |
| `npm run lint`                 | ESLint                                              |
| `npm run typecheck`            | `tsc --noEmit`                                      |
| `npm run test:run`             | Testes uma vez                                      |
| `npm run build`                | Build de produção                                   |
| `npm run db:start` / `db:stop` | Sobe e derruba o Supabase local (precisa de Docker) |
| `npm run db:reset`             | Recria o banco local a partir das migrations        |
| `npm run db:diff`              | Compara o banco local com produção                  |
| `npm run db:push`              | Aplica migrations pendentes em produção             |
| `npm run db:types`             | Regenera `src/types/database.types.ts`              |

Antes de dizer que terminou: `npm run lint && npm run typecheck && npm run test:run && npm run build`.

## Estrutura

- `src/app/` — roteador, providers, guarda de autenticação, layout
- `src/domain/` — regras puras com teste, sem React e sem Supabase
- `src/lib/` — infraestrutura: cliente Supabase, cache, log, tradução de erro
- `src/features/<feature>/` — `api.ts`, `queries.ts` e telas de cada área
- `src/components/ui/` — componentes do shadcn
- `src/components/` — telas antigas em `.jsx`, migradas conforme forem reescritas
- `src/types/database.types.ts` — gerado pelo Supabase, nunca editar à mão
- `supabase/migrations/` — única fonte de verdade do schema

## Convenções

- Regra de negócio vive em `src/domain` ou no banco, nunca na tela.
- Dinheiro é `number` em reais nesta fase; a Fase 2 troca para inteiro em centavos.
- Datas sempre por `src/domain/dates.ts`. Nunca `new Date('YYYY-MM-DD')` nem `toISOString().split('T')[0]`.
- Toda tabela tem RLS com `user_id = auth.uid()`. Nenhuma função aceita `user_id` do cliente.
- Busca de dados por TanStack Query. Nada de `useEffect` com `fetch` em tela nova.
- Teste antes da implementação para qualquer regra de domínio.
- Commits em português, `tipo: descrição`.

## Ao mexer em dinheiro, fatura, parcela, saldo, dívida ou dado pessoal

Use o agente `sofinance-senior-engineer` e a skill `sofinance-domain`.

## Onde está a documentação

- Plano geral: `docs/superpowers/specs/2026-09-17-sofinance-revamp-design.md`
- Planos por fase: `docs/superpowers/plans/`
- SQL histórico que não reflete produção: `docs/legacy-sql/` (não aplicar)
```

- [ ] **Step 2: Escrever a skill de domínio**

Create `.claude/skills/sofinance-domain/SKILL.md`:

```markdown
---
name: sofinance-domain
description: Regras de negócio financeiras do Sofinance (fatura de cartão, parcelas, saldo, recorrências, dívidas SAC e Price, orçamento, LGPD e formatação brasileira). Use sempre que a tarefa tocar dinheiro, fatura, parcela, saldo, vencimento, dívida ou dado pessoal.
---

# Regras de negócio do Sofinance

## Contas e saldo

Saldo de conta = saldo inicial + entradas pagas − saídas pagas ± transferências. Lançamento pendente entra só no saldo projetado. Transferência tem origem e destino diferentes e não conta como receita nem como despesa nos relatórios.

## Cartão de crédito (modelo "fatura como conta a pagar")

Criar um cartão exige apenas um nome. Cor, dia de vencimento, dia de fechamento e conta pagadora são opcionais. Não existe limite de crédito no modelo.

Todo mês o sistema cria uma fatura por cartão como conta a pagar. O usuário informa o valor total quando a fatura chega; pagar gera uma despesa na categoria "Cartão de Crédito" debitada da conta escolhida.

Detalhar a fatura é opcional. Relatórios por categoria usam os itens quando existem e o total informado quando não existem. Nunca somar os dois.

Com dia de fechamento informado: item com data entre o fechamento anterior (exclusivo) e o fechamento atual (inclusivo) cai na fatura que vence no dia de vencimento seguinte. Se o vencimento for menor que o fechamento, a fatura vence no mês seguinte ao fechamento. Sem fechamento informado, o item cai na fatura do mês da data.

Parcelamento em N vezes cria N itens em faturas consecutivas, com arredondamento em centavos e a última parcela absorvendo a diferença.

Exemplo canônico: cartão fecha dia 15 e vence dia 25. Compra em 16/03 entra na fatura que vence em 25/04.

## Recorrências

Geram lançamentos pendentes por materialização diária, nunca calculados na hora da renderização. Pagar é marcar como pago. Frequências: mensal, semanal, anual. Dia 29, 30 ou 31 em mês curto cai no último dia do mês.

## Dívidas

Tipos: imóvel, veículo, empréstimo, consórcio, outro. Sistemas: Price (parcela fixa), SAC (amortização fixa) e fixo (parcela informada). Imóvel no Brasil costuma ser SAC com TR; veículo costuma ser Price. Taxa informada ao ano converte para mês por `(1 + i)^(1/12) − 1`. Saldo devedor nunca é parcelas restantes vezes valor da parcela em SAC ou Price.

## Orçamento e metas

Orçamento por categoria compara gasto pago mais pendente do mês contra o limite, com alerta em 80% e 100%. Meta tem histórico de aportes; progresso é a soma dos aportes dividida pelo valor alvo.

## Formatação

`R$ 1.234,56`; datas `dd/MM/yyyy`; mês de referência `YYYY-MM`; `Intl.NumberFormat('pt-BR')`. Fuso `America/Sao_Paulo`.

## LGPD

Consentimento explícito no cadastro, exportação dos dados e exclusão completa (dados, storage e `auth.users`) por Edge Function com service role. Nunca expor a chave `service_role` no cliente.
```

- [ ] **Step 3: Atualizar o agente sênior**

Em `.claude/agents/sofinance-senior-engineer.md`, troque a linha que manda ler `database/` e `supabase-setup.sql` por uma que aponte para `supabase/migrations/`, e acrescente uma linha mandando consultar a skill `sofinance-domain` antes de decidir regra de negócio.

- [ ] **Step 4: Atualizar o README**

Em `README.md`, na seção de instalação, acrescente os passos do banco local:

````markdown
### Banco local (opcional, precisa de Docker)

```bash
npm run db:start   # sobe Postgres, Auth e API locais
npm run db:reset   # recria o banco a partir de supabase/migrations
npm run db:stop
```
````

````

E na seção de estrutura do projeto, substitua a árvore antiga pela estrutura descrita no `CLAUDE.md`.

- [ ] **Step 5: Registrar a fase no spec**

Em `docs/superpowers/specs/2026-09-17-sofinance-revamp-design.md`, logo depois da seção `13.3.2`, acrescente uma seção `13.3.3 Fase 1 executada` com a data, a lista do que foi entregue e as decisões de versão da tabela no topo deste plano.

- [ ] **Step 6: Verificação final**

```bash
npm run lint && npm run typecheck && npm run test:run && npm run build
grep -rn "activeSection\|setActiveSection" src || echo "navegação antiga removida"
grep -rn "from '@/utils/" src || echo "sem imports de src/utils"
npx supabase db diff --linked --schema public
````

Expected: tudo passa, as duas buscas não retornam nada e o diff do banco vem vazio.

- [ ] **Step 7: Revisão de segurança**

Invoque a skill `security-review` sobre a branch. Corrija qualquer achado de severidade alta antes de seguir.

- [ ] **Step 8: Revisão de código**

Invoque `superpowers:requesting-code-review` com o intervalo `main..fase-1/fundacao`. Aplique as correções e commite.

- [ ] **Step 9: Integrar**

Siga `superpowers:finishing-a-development-branch`. A Vercel publica sozinha depois do push para `main`. Depois do deploy, repita no site em produção os itens 1 a 6 do Step 11 da Task 9.

---

## Pendências que dependem do Iago

| Item                            | Quando                 | Onde                                   |
| ------------------------------- | ---------------------- | -------------------------------------- |
| Senha do banco para o `db pull` | Task 1                 | Supabase → Project Settings → Database |
| Docker Desktop aberto e rodando | Task 1                 | Já instalado                           |
| Aprovar o merge em `main`       | Task 13                | Git                                    |
| DSN do Sentry na Vercel         | Antes do beta (Fase 4) | sentry.io e Vercel                     |

## Correções aplicadas durante a execução

Registradas aqui porque o plano é lido depois por quem não acompanhou a execução, e um plano que mente é pior que um plano ausente.

| Onde    | O que estava errado                                        | Correção                                                                                                                                                                                                       |
| ------- | ---------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Task 3  | `manualChunks` na forma de objeto                          | O Vite 8 usa Rolldown e só aceita função                                                                                                                                                                       |
| Task 4  | `tsconfig.node.json` com `noEmit` e `composite` juntos     | O TypeScript rejeita com TS6310; `noEmit` sai                                                                                                                                                                  |
| Task 5  | `settings.react.version: 'detect'`                         | Quebra o ESLint 10, porque o eslint-plugin-react 7.37.5 chama `context.getFilename()`, removido na 10. Fixado em `'19.3.0'`                                                                                    |
| Task 6  | Gatilho só em `main` e pull request                        | O próprio passo de verificação conferia a branch de trabalho, que nada dispararia. Passa a rodar em toda branch, com `concurrency`                                                                             |
| Task 6  | `actions/checkout@v5` e `actions/setup-node@v5`            | Dois majors vencidos; as releases atuais são v7.0.1 e v7.0.0                                                                                                                                                   |
| Task 6  | `gh run list` no passo de verificação                      | O `gh` não está instalado; a conferência passa a ler a API pública do GitHub                                                                                                                                   |
| Task 7  | `diasAteVencimento(diaVencimento: number, …)`              | O bloco Interfaces se contradizia: o teste já existente chama com `undefined`, e o `ContaComVencimento.dia_vencimento: number \| null` do mesmo brief desagua na função. Passa a `number \| null \| undefined` |
| Task 8  | `grep -rln "services/supabaseClient" src`                  | Achava 5 dos 11 importadores: os 6 servicos usam `'./supabaseClient'` e escapavam do padrao                                                                                                                    |
| Task 8  | `ContaFixaInput` e `MetaInput` iguais ao `Insert` do banco | O `Insert` traz `user_id` obrigatorio, que o servico sobrescreve com o da sessao. Passam a `Omit<…, 'user_id'>`, senao a Task 12 tiparia o formulario pedindo o campo que o chamador nunca deve informar       |
| Task 11 | `shadcn init -y -b radix -t vite`                          | O CLI atual (4.21.0) abre seletor interativo de preset e ignora o `-y`; `-t` serve para criar projeto novo. A forma que funciona e `-p nova --no-monorepo`                                                     |
| Task 11 | Step 8 conferia so linhas e `glass-card`                   | Nao detectaria troca de VALOR de variavel, e o `shadcn init` sobrescreveu `--primary` e `--secondary` no `:root` existente. A conferencia certa e comparar o conjunto de variaveis do `:root` antes e depois   |
| Task 11 | Nada previa o conflito de caixa de `src/components/ui`     | O git tinha a pasta como `UI`; no Windows e a mesma do `ui` minusculo, no Linux do CI nao. Renomeada antes do shadcn, em commit separado                                                                       |

Fora do plano, a execução expôs um defeito que nenhuma verificação local pegaria: um `npm ci` limpo falhava com ERESOLVE, porque três plugins do ESLint ainda não declaram suporte ao ESLint 10. O `node_modules` da máquina era anterior à Task 5 e nunca tinha passado por resolução do zero, então o repositório estava quebrado para qualquer clone novo sem que nada acusasse. Resolvido com um `overrides` restrito a esses três pacotes, na Task 6.
