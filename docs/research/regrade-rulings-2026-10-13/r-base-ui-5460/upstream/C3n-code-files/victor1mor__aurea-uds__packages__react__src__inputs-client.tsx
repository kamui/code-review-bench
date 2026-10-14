"use client";
// Fase 9 (achado A5): este arquivo saiu do index.tsx de 970 linhas. Um módulo por categoria
// do registry — a taxonomia já existia e é gateada. A ordem de import entre eles é um DAG:
// internal → system → actions → feedback → inputs → navigation → layout → data-display → resto.
import React, {forwardRef, type ButtonHTMLAttributes, type HTMLAttributes, type InputHTMLAttributes, type SelectHTMLAttributes, type TextareaHTMLAttributes, type RefAttributes, type ReactNode, type ReactElement} from "react";
import {valorBase, peleDoEixo, soOValor, type Responsive} from "./pure.js";
import {Combobox as BaseCombobox} from "@base-ui/react/combobox";
import {Select as BaseSelect} from "@base-ui/react/select";
import {Form as BaseForm} from "@base-ui/react/form";
import {Field as BaseField} from "@base-ui/react/field";
import {NumberField as BaseNumberField} from "@base-ui/react/number-field";
import {OTPField as BaseOTPField} from "@base-ui/react/otp-field";
import {RadioGroup as BaseRadioGroup} from "@base-ui/react/radio-group";
import {Radio as BaseRadio} from "@base-ui/react/radio";
import {cx, useAureaStrings, usePortalContainer, useReorder} from "./internal.js";
// Os seis controles abaixo saíram deste arquivo no item O1 e moram no `markup.tsx`, sem a
// diretiva — são marcação, e quem escuta `onChange` é o consumidor. Voltam IMPORTADOS e não só
// reexportados porque o `ehControle` do `Field` compara por IDENTIDADE de referência: dois
// objetos diferentes com o mesmo nome quebrariam o rótulo do campo em silêncio.
import {Textarea, Checkbox, Radio, Switch, Range, InputGroup, InputGroupAddon, type FieldSize} from "./markup.js";
import {Icon, type IconName} from "./system.js";
import {IconButton} from "./actions.js";
// FileInput mora em arquivo próprio (132 linhas — upload real, aborto, progresso) e é público
// por aqui: a categoria dele é Inputs.
export {FileInput, matchesAccept, type FileRejection, type UploadContext, type UploadFn} from "./file-input.js";

// `id`, quando vem, é o id do CONTROLE — não deste contêiner. É o que um consumidor quer dizer
// ao passá-lo, e é o que faz `htmlFor` apontar para o lugar certo. O `htmlFor` que este tipo
// declarava saiu: o contêiner deixou de ser `<label>`, então não havia mais o que ele nomeasse.
// `orientation` (G-AXIS-01): a linha do formulário de configurações — rótulo à esquerda, controle
// à direita — só saía da Aurea com CSS avulso. A pele (`.field-horizontal`) veio no core pelo
// merge; a prop faltava. É visual puro: não muda ordem, semântica nem teclado, então resolve por
// classe, como manda a ADR-0047.
export type FieldOrientation="vertical"|"horizontal";
// `labelWidth` (C-10, 24/09/2026): no `horizontal` a coluna do rótulo é o token
// `--field-label-width` (12rem, 192px), e não havia prop para ele. "Ordenar" (60px) deixava 130px
// vazios, e num telefone a 375px o campo ficava com 85px — 13 de respiro e 70 dos botões, ZERO para
// o texto: o valor escolhido sumia. Medido num app real com a 0.8.8. `"auto"` faz a coluna do
// tamanho do rótulo; uma medida CSS ("8rem") fixa outra. Sem a prop, os 192px de sempre. A prop só
// escreve o token que o core já lê — nenhuma regra nova de CSS.
export type FieldLabelWidth="auto"|(string&{});
export interface FieldProps extends HTMLAttributes<HTMLDivElement>,RefAttributes<HTMLDivElement>{label:string;hint?:ReactNode;error?:ReactNode;
  /** B-12 (24/09/2026): texto de ajuda que precisa de uma FRASE — mora embaixo do controle, onde
   *  cabe, e não divide a linha do rótulo. O `hint` continua o que é: nota curta ao lado do rótulo
   *  ("opcional", "em MB"). É o lugar da descrição da referência e da `description` que o
   *  `Checkbox` desta casa já tem. Entra no `aria-describedby` na ordem da tela: hint, descrição,
   *  erro. */
  description?:ReactNode;name?:string;orientation?:Responsive<FieldOrientation>;labelWidth?:FieldLabelWidth}
// hint e error são DESCRIÇÃO (aria-describedby), nunca parte do NOME. Nome é o que o campo É;
// hint e erro são sobre o VALOR.
//
// ISTO JÁ ESTAVA ESCRITO AQUI E ERA MENTIRA — reaberto como AUD-0001 em 12/08/2026, e o achado
// A12 volta com ele. O comentário anterior dizia "medido em 30/07/2026, corrigido", e a marcação
// logo abaixo mantinha `hint` e `error` DENTRO do `<label>` externo. Nome acessível medido:
// "E-mail Usamos para entrar Endereço inválido" — os três grudados —, e a descrição repetindo o
// que já estava no nome. Quinta vez neste repositório que comentário passou por código; a
// diferença é que aqui ele passou por CORREÇÃO. O teste que devia pegar aceitava `/E-mail/`, que
// casa com a frase inteira: regex frouxa é gate que dorme.
//
// O contêiner é `<div>` e não `<label>`, e isso resolve três coisas de uma vez: hint e erro saem
// da árvore de nome; `Checkbox`/`Radio`/`Switch`, que trazem `<label>` próprio, param de produzir
// `<label>` dentro de `<label>` (HTML inválido); e a ligação passa a ser explícita.
//
// DUAS FORMAS, e a regra que escolhe entre elas é UMA: o Field só é dono do NOME quando há um
// filho único que não se nomeia sozinho.
//   • dono do nome  → `<label htmlFor>`, e o filho recebe `id`/`aria-describedby`/`aria-invalid`;
//   • não dono      → `role="group"` + `aria-labelledby`, sem `htmlFor`.
// Os dois casos de "não dono" foram medidos, não imaginados:
//   1. VÁRIOS FILHOS — o catálogo já faz isso (um grupo de rádios sob um rótulo). Aqui `htmlFor`
//      apontaria para um id que não existe, o que é pior que não ligar.
//   2. FILHO COM RÓTULO PRÓPRIO — `Switch`, `Checkbox`, `Radio` trazem `<label>` deles. Somar um
//      `<label for>` daria DOIS rótulos ao mesmo controle, e o nome acessível vira a concatenação
//      dos dois ("Notificações Ativo"). A detecção é a prop `label` do filho: é prop NOSSA, não
//      é farejar tipo — todo controle que se nomeia declara ela.
// O `role="group"` não é invenção: é o que a ficha do Field sempre declarou.
//
// LIMITE DECLARADO, e é o que faz o clone falhar em silêncio: componente que destrincha uma
// lista fixa de props DESCARTA o que for injetado. `Input`, `Select`, `Textarea`, `Range` e
// `Switch` espalham `...props` e recebem tudo. `Combobox` e `MultiCombobox` não espalhavam, e
// por isso ganharam `aria-describedby`/`aria-invalid` explícitos abaixo. Os outros
// (`NumberField`, `OTPField`, `SegmentedControl`, `Toggle`) têm rótulo próprio e não são filhos
// de `Field` — a ficha diz quais são.
// As tags que o HTML permite associar a um `<label>`. Fora desta lista (e fora dos nossos
// componentes), o filho é layout e o Field vira grupo — ver o comentário dentro da função.
const TAGS_ROTULAVEIS=new Set(["input","select","textarea","button","meter","output","progress"]);
function filhoUnico(children:ReactNode):ReactElement<Record<string,unknown>>|null{
  const filhos=React.Children.toArray(children);
  if(filhos.length!==1||!React.isValidElement(filhos[0]))return null;
  const filho=filhos[0] as ReactElement<Record<string,unknown>>;
  return filho.type===React.Fragment?filhoUnico(filho.props.children as ReactNode):filho;
}
function elementoRotulavel(tipo:ReactElement["type"]):boolean{
  if(typeof tipo==="string")return TAGS_ROTULAVEIS.has(tipo);
  // Componente React arbitrário pode consumir as props no wrapper e nunca entregá-las ao nó
  // focal. Só os controles Aurea cujo encaminhamento é parte do contrato entram aqui.
  // `PasswordField` entrou em 29/08/2026, e entrou medido: `<Field label="Senha"><PasswordField/></Field>`
  // reprovava no axe com "Form elements must have labels". Fora desta lista o Field vira GRUPO e
  // não pendura `htmlFor`, então o <input> de dentro ficava sem nome. Ele qualifica pela mesma
  // regra que os outros: recebe `id` e espalha `...props` no `Input`, que é o nó focal.
  return ([Input,Select,Textarea,SearchField,PasswordField,Checkbox,Radio,Switch,Range,Combobox,MultiCombobox] as unknown[]).includes(tipo);
}
export function Field({label,hint,description,error,children,className,id,name,orientation,labelWidth,style,...props}:FieldProps){
  const auto=React.useId();
  // Fragment não cria nó no DOM. Um Fragment com um só controle é só sintaxe; com vários,
  // Field precisa voltar para grupo em vez de pendurar `htmlFor` num id que nunca existirá.
  const unico=filhoUnico(children);
  // e o TERCEIRO caso de "não dono", que o gate achou e eu não tinha previsto: o filho único pode
  // ser um `<div>` de LAYOUT com os controles dentro — o catálogo faz isso no `Range` ("With the
  // number": um slider e um campo numérico lado a lado). `htmlFor` apontando para um `<div>` não
  // nomeia nada, e o `catalog-sweep` reprovou `range.html` com duas violações de `label` no axe.
  // A leitura é do TIPO do elemento JSX, que é API estável do React. Para tag nativa vale a lista
  // do HTML; para componente vale a lista explícita dos controles Aurea que repassam a ligação ao
  // nó focal. Componente arbitrário vira grupo — presumir forwarding recria o id sem alvo.
  const rotulavel=!!unico&&elementoRotulavel(unico.type);
  // `label` é a API dos controles compostos; aria-label/labelledby são a API universal. Se uma
  // delas já nomeia o filho, acrescentar `<label for>` daria dois donos para o mesmo nome.
  const nomeProprio=rotulavel&&(unico!.props.label!==undefined||[unico!.props["aria-label"],unico!.props["aria-labelledby"]].some(v=>typeof v==="string"&&v.trim().length>0));
  const nomeAqui=rotulavel&&!nomeProprio;
  // o id que o filho já declara VENCE: sobrescrevê-lo quebraria uma ligação que o consumidor fez.
  const idControle=(unico?.props.id as string|undefined)??id??`${auto}-control`;
  const idRotulo=`${auto}-label`;
  const idHint=hint?`${auto}-hint`:undefined;
  const idDescricao=description?`${auto}-description`:undefined;
  const idErro=error?`${auto}-error`:undefined;
  const descrito=[idHint,idDescricao,idErro].filter(Boolean).join(" ")||undefined;
  // As chaves de aria só entram quando têm VALOR. Passar `undefined` explícito não é o mesmo que
  // não passar: quando o filho vira `Field.Control` do motor (ramo do `name`), a prop explícita
  // do elemento vence a mesclagem, e o `aria-invalid` que o motor calculou a partir do erro
  // externo era apagado por um `undefined` nosso. Medido em 13/08/2026: o input saía com
  // `data-invalid` e SEM `aria-invalid`, ou seja, vermelho na tela e válido para o leitor de
  // tela — que é o pior dos dois mundos.
  const injecao:Record<string,unknown>={id:idControle};
  const descritoFinal=[unico?.props["aria-describedby"],descrito].filter(Boolean).join(" ");
  if(descritoFinal)injecao["aria-describedby"]=descritoFinal;
  const invalidoFinal=error?true:unico?.props["aria-invalid"];
  if(invalidoFinal!==undefined)injecao["aria-invalid"]=invalidoFinal;
  const controle=rotulavel?React.cloneElement(unico!,injecao):children;
  // a descrição mora no nó FOCAL quando existe um; sem filho único, no grupo — que é o que sobra
  // para anunciar. Nos dois lugares seria anunciar duas vezes.
  // Com `name`, o filho passa a ser o CONTROLE do motor: é isso que faz o `aria-invalid` e o
  // `aria-describedby` do erro externo chegarem ao nó focal. Medido em 13/08/2026 — sem este
  // embrulho o `Field.Root` recebe o erro e não tem a quem entregá-lo, e o campo fica válido na
  // árvore de acessibilidade enquanto a frase de erro aparece na tela. Pior que não mostrar.
  const controleFinal=name?<BaseField.Control render={controle as ReactElement}/>:controle;
  const estilo=labelWidth?{...style,"--field-label-width":labelWidth==="auto"?"max-content":labelWidth} as React.CSSProperties:style;
  const corpo=<div className={cx("field",peleDoEixo("field",orientation,"vertical","field"),className)} {...(nomeAqui?{}:{role:"group","aria-labelledby":idRotulo,...(rotulavel?{}:{"aria-describedby":descrito})})} style={estilo} {...props}>
    <span className="label">
      {nomeAqui?<label htmlFor={idControle}>{label}</label>:<span id={idRotulo}>{label}</span>}
      {hint&&<span className="hint" id={idHint}>{hint}</span>}
    </span>
    {controleFinal}
    {description&&<span className="field-description" id={idDescricao}>{description}</span>}
    {error&&<span className="field-error" id={idErro}>{error}</span>}
    {/* O erro que veio DE FORA (servidor, action, schema) — o `Form` o entrega por `name`, e é
        este nó que o motor preenche. Sem `name` ele não existe, e o `Field` continua sendo o de
        antes byte a byte: a mudança é aditiva de propósito, porque a semântica deste componente
        foi paga com auditoria (AUD-0001) e não se mexe nela para acrescentar recurso. */}
    {name&&<BaseField.Error className="field-error"/>}
  </div>;
  // `Field.Root` do motor entra SÓ quando há `name`: é ele que liga o campo à chave de `errors`
  // do formulário. Ele não desenha nada — é contexto — então a pele não muda.
  return name?<BaseField.Root name={name} invalid={error?true:undefined} render={corpo}/>:corpo;
}

// Form (M1): o formulário que EXIBE erro, e não o que decide o que é erro.
//
// A divisão saiu de pesquisa e da medição dos consumidores (13/08/2026). O padrão de mercado é
// uma biblioteca de formulário com uma de esquema, e é o que uma das referências embrulha — mas embrulhar isso aqui obrigaria TODO
// projeto que usa a Aurea a usar essa biblioteca de formulário, inclusive os que já validam de outro jeito (um
// consumidor real valida com a biblioteca de esquema em server action, sem biblioteca de formulário no cliente).
// Dependência nova também é decisão do Victor pelo BUILDING.md §3.3, e ela não foi necessária:
// o `Form` do Base UI — motor que a Aurea JÁ paga desde a Fase 2 — declara no próprio tipo que
// `errors` são "validation errors returned externally, typically after submission by a server or
// a form action". Ou seja, a divisão que eu ia propor já é a do motor.
//
// O que a Aurea entrega: a pele, o `name` ligando campo a erro, e o envio com os valores já
// coletados. O que ela NÃO entrega, e não vai: schema, regra, resolver, estado de campo.
export function Form({errors,onSubmit,children,className,...props}:Omit<React.FormHTMLAttributes<HTMLFormElement>,"onSubmit">&RefAttributes<HTMLFormElement>&{errors?:Record<string,string|string[]>;onSubmit?:(values:Record<string,unknown>)=>void}){
  return <BaseForm errors={errors} onFormSubmit={onSubmit?(v:Record<string,unknown>)=>onSubmit(v):undefined} className={cx("form",className)} {...props}>{children}</BaseForm>;
}
// `formatOnBlur` é a metade de TEXTO do item L6 (máscara no campo), e a decisão de NÃO mascarar
// enquanto se digita é pesquisada, não preferida — três medições independentes, em 15/08/2026:
//   • a máscara de entrada de um **design system público** é publicada com
//     reprovação registrada em WCAG 2.1 AA: sair de um erro fica difícil por falta de
//     retorno. Não é que ninguém tenha feito; é que quem fez, publicou o defeito junto;
//   • uma das **referências** ABANDONOU máscara nos campos de data e escreveu por quê: o texto vaza
//     para as seções anteriores ao editar o meio do valor. Há até um vídeo no repositório deles
//     mostrando a máscara como experiência ruim — foi a referência que o item mandava ler;
//   • a prática corrente diz o mesmo: máscara ao vivo descasa o que o leitor de tela ANUNCIA (o
//     que foi digitado) do que o campo MOSTRA (o que a máscara deixou), e a recomendação é deixar
//     digitar e colar à vontade, formatando DEPOIS que o foco sai.
//
// Então a Aurea entrega o MOMENTO, que é a decisão de acessibilidade, e não o formato — placa,
// documento e telefone são regra de país, conhecimento do consumidor, não do design system.
//
// A TRAVA DO ITEM ESTÁ SATISFEITA POR CONSTRUÇÃO: o valor formatado é o `value` do elemento, no
// DOM. Nada é pintado por cima, nada mora só na tela — é o que o teste cobra.
//
// O `onChange` é chamado DEPOIS de o valor do elemento já ter mudado, e é isso que faz o campo
// controlado funcionar: o consumidor lê `e.target.value` e recebe o texto formatado. O evento é
// de blur, e isso está declarado — quem ramifica por `e.type` precisa saber.
// O DEGRAU DE TAMANHO (merge de 28/08/2026): a página deste componente já PROMETIA que `size` é
// o degrau do sistema (sm/md/lg) e não a contagem de caracteres do HTML — e o componente passava
// o atributo nativo adiante, então os três campos de exemplo saíam do mesmo tamanho. Promessa na
// documentação e nada no código. `Omit<…,"size">` faz o que a documentação já dizia.
export interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>,"size">,RefAttributes<HTMLInputElement>{formatOnBlur?:(value:string)=>string;size?:Responsive<FieldSize>}
export const Input=forwardRef<HTMLInputElement,InputProps>(function Input({className,formatOnBlur,size,onBlur,onChange,...props},ref){
return <input ref={ref} className={cx("input",peleDoEixo("input",size),className)} onChange={onChange} onBlur={e=>{
  if(formatOnBlur){
    const formatado=formatOnBlur(e.currentTarget.value);
    if(formatado!==e.currentTarget.value){
      e.currentTarget.value=formatado;
      onChange?.(e as unknown as React.ChangeEvent<HTMLInputElement>);
    }
  }
  onBlur?.(e);
}} {...props}/>});


// O mesmo degrau do `Input`, porque ele É um `Input` por dentro: sem o `Omit` o tipo nativo
// (`size: number`) chega ao componente que hoje espera o degrau, e o compilador reprova — foi
// exatamente assim que este sítio apareceu.
export interface SearchFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>,"size"|"width">,RefAttributes<HTMLInputElement>{icon?:IconName;size?:Responsive<FieldSize>;
  /** A-08 (24/09/2026): a largura do campo — `"20rem"` ou `"auto"`. Sem ela, 100% da linha, como
   *  sempre. O `size` muda altura e letra, nunca largura. */
  width?:string}
// O glifo entra por `InputGroup`/`InputGroupAddon`, e não pelo `.input-wrap`: o mesmo problema
// tinha sido resolvido DUAS vezes localmente aqui — `.input-wrap` para o glifo do SearchField e
// `.input-wrap-end` para o botão do PasswordField —, e o grupo é a terceira vez virando uma. O
// merge das duas linhagens tinha trazido de volta a versão com `.input-wrap`, contra um core
// que já não a pinta.
export function SearchField({icon="magnifying-glass",className,size,width,...props}:SearchFieldProps){return <InputGroup className={className} width={width}><InputGroupAddon><Icon name={icon} size={valorBase(size)==="sm"?"sm":undefined}/></InputGroupAddon><Input type="search" size={size} {...props}/></InputGroup>}
// labelHidden: coluna de seleção de tabela precisa do rótulo POR LINHA para o leitor de
// tela ("Select Analyst"), mas mostrá-lo engorda a coluna. O rótulo continua no DOM, só
// sai da tela (.sr-only) — nunca trocar por aria-label solto num <label> visível vazio.





// NumberField (Lote 1 do BUILDING.md). Um `<input type="number">` cru tem três problemas que
// este resolve, e todos vieram do motor, não de nós: as setas nativas são alvos minúsculos e
// somem no Safari; a roda do mouse altera o valor por acidente sobre o campo focado; e o valor
// digitado não é formatado por locale. O Base UI trata os três.
// A anatomia (menos · campo · mais numa peça só) é a que as referências convergem.
// Fora de propósito: o ScrubArea do Base UI — arrastar o rótulo para variar o número. É gesto
// que ninguém descobre sozinho e que não tem equivalente por teclado.
// `format`/`locale`/`name` são a metade de MOEDA do item L6, e não custaram componente nenhum: o
// comentário acima já dizia que o motor formata por locale, e a nossa casca simplesmente não
// repassava a opção. Medido no `@base-ui/react@1.6.0`: `NumberFieldRoot` aceita
// `format?: Intl.NumberFormatOptions` e `locale?`, e formata no BLUR — que é exatamente o momento
// que a pesquisa de acessibilidade recomenda, e o oposto da máscara ao vivo.
// Com isso, `<NumberField format={{style:"currency",currency:"BRL"}} locale="pt-BR"/>` substitui a
// biblioteca de moeda que um consumidor carrega só para isso. Quem formata é o `Intl` da
// plataforma; a Aurea não escreve formatador de dinheiro, e nem deveria.
// `name` entra junto porque é ele que faz o motor renderizar o input escondido com o valor CRU —
// é a trava do item ("o valor mascarado continua no DOM") entregue pelo motor, e o teste a cobra.
export function NumberField({value,defaultValue,onValueChange,min,max,step,disabled,readOnly,required,label,id,name,format,locale,className,scrubbable,scrubDirection}:{value?:number|null;defaultValue?:number;onValueChange?:(v:number|null)=>void;min?:number;max?:number;step?:number;disabled?:boolean;readOnly?:boolean;required?:boolean;label?:string;id?:string;name?:string;format?:Intl.NumberFormatOptions;locale?:string;className?:string;scrubbable?:boolean;scrubDirection?:"horizontal"|"vertical"}){
const s=useAureaStrings();
return <BaseNumberField.Root id={id} name={name} value={value} defaultValue={defaultValue} onValueChange={soOValor(onValueChange)} min={min} max={max} step={step} format={format} locale={locale} disabled={disabled} readOnly={readOnly} required={required} className={cx("number-field",className)}>
<BaseNumberField.Group className="number-field-group">
{/* ── REPORTADA no merge de 28/08/2026 ─────────────────────────────────────────────────────
    A alça de arrasto vivia na outra linhagem e sumiu quando este arquivo entrou inteiro da
    `main`. A perda foi SILENCIOSA — a conferência comparou NOMES de componente, e
    `NumberField` existe nos dois lados; o que mudou foi o corpo. Quem denunciou foi o gate da
    fronteira do core: `.number-field-scrub` ficou no CSS sem ninguém a emitir.

    `scrubbable` é OPT-IN e SOMA em vez de substituir: os dois botões, as setas e Home/End continuam intactos. O
    arrasto é atalho de PONTEIRO por cima de um caminho que já existe inteiro.
    `aria-hidden` na alça porque ela não é operável por teclado nem por leitor de tela, e
    anunciar um alvo que a tecnologia assistiva não alcança é pior que não anunciar.
    O eixo sai como CLASSE porque o motor não publica atributo para ele — a regra da casa é
    atributo quando o motor publica, classe quando não.
    E o cursor virtual PRECISA de conteúdo: o Pointer Lock esconde o ponteiro do sistema e
    desenha este elemento no lugar; vazio, o arraste fica sem cursor nenhum. */}
{scrubbable&&<BaseNumberField.ScrubArea className={cx("number-field-scrub",scrubDirection==="vertical"&&"number-field-scrub-vertical")} direction={scrubDirection} aria-hidden="true">
  <Icon name="dots-six-vertical" aria-hidden={true}/>
  <BaseNumberField.ScrubAreaCursor className="number-field-scrub-cursor"><Icon name="dots-six-vertical" aria-hidden={true}/></BaseNumberField.ScrubAreaCursor>
</BaseNumberField.ScrubArea>}
<BaseNumberField.Decrement className="btn btn-ghost btn-icon" aria-label={s.decrement}><Icon name="minus"/></BaseNumberField.Decrement>
<BaseNumberField.Input className="input number-field-input" aria-label={label}/>
<BaseNumberField.Increment className="btn btn-ghost btn-icon" aria-label={s.increment}><Icon name="plus"/></BaseNumberField.Increment>
</BaseNumberField.Group></BaseNumberField.Root>}

// OTPField (Lote 1 do BUILDING.md). Um campo por dígito, com o comportamento que ninguém acerta
// à mão: colar o código inteiro distribui pelos campos, Backspace volta um, e o
// `autocomplete="one-time-code"` deixa o iOS oferecer o código do SMS. Tudo do motor.
// Uma referência resolve isto com um pacote npm separado; aqui não entra dependência nova —
// o `@base-ui/react`, que já é a única dependência de runtime da biblioteca, tem `otp-field`.
// `length` é obrigatório de propósito: sem ele o campo não sabe quando está completo.
// CADA caixa leva nome próprio, e não só o grupo. O `catalog-sweep` pegou isto no primeiro
// lote: com `aria-label` só na raiz, o axe acusou `label` em todas as seis — seis campos de
// formulário anônimos. Um leitor de tela anunciaria "editar texto" seis vezes seguidas sem
// dizer qual é qual. O grupo continua nomeado (`role="group"`), que é o que diz para que serve
// o código; o nome de cada caixa é o que diz onde você está.
export function OTPField({length,value,defaultValue,onValueChange,mask,disabled,required,label,id,className}:{length:number;value?:string;defaultValue?:string;onValueChange?:(v:string)=>void;mask?:boolean;disabled?:boolean;required?:boolean;label?:string;id?:string;className?:string}){
const s=useAureaStrings();
return <BaseOTPField.Root id={id} length={length} value={value} defaultValue={defaultValue} onValueChange={soOValor(onValueChange)} mask={mask} disabled={disabled} required={required} role="group" aria-label={label} className={cx("otp-field",className)}>
{/* Duas coisas medidas no Base UI 1.6.0, e as duas custaram uma volta:
    1. O índice de cada campo é do MOTOR, pela ordem no DOM — `index` não é prop.
    2. `aria-label` passado direto no `Input` é DESCARTADO no PRIMEIRO campo (só nele), com ou
       sem rótulo na raiz. Medido: o axe acusou `label` justamente nele. Pelo `render` o
       atributo sobrevive, e todos os outros que o motor injeta (autocomplete=one-time-code,
       pattern, enterKeyHint, tabindex) continuam intactos — conferido no markup emitido. */}
{Array.from({length},(_,i)=><BaseOTPField.Input key={i} render={<input aria-label={`${s.otpDigit} ${i+1}`}/>} className="input otp-slot"/>)}
</BaseOTPField.Root>}
// SegmentedControl — achado M20, fechado na Parte C do PLANO-1.0 em 07/08/2026.
//
// O DEFEITO: era `role="group"` com N botões de `aria-pressed`. Isso descreve N alternâncias
// INDEPENDENTES — cada botão anuncia "pressionado/não pressionado", nada diz que só um pode
// valer, e nunca se ouve "1 de 2". Para escolha única entre poucas opções o padrão APG é
// `radiogroup`, e a diferença é o que o leitor de tela consegue prometer.
// A troca mudou a semântica de um componente publicado, então foi registrada: ADR-0016.
//
// O MOTOR ENTREGA, ENTÃO NÃO ESCREVEMOS (BUILDING.md §1). O `RadioGroup` do Base UI 1.6 traz o
// padrão inteiro: roving tabindex, setas que movem E selecionam, Home/End, e `aria-checked`.
// Escrever isso à mão seria reimplementar um composite que já está testado.
//
// `render={<button type="button"/>}` não é enfeite, e a medição de 07/08 é que disse: o
// `Radio.Root` renderiza um `<span>` por padrão, e a pele da Aurea é `.segmented button`. Sem
// isso o componente perderia a pele inteira — e o check 18 não veria, porque ele olha CLASSE.
// O motor também emite um `<input type="radio">` escondido por item (para envio de formulário);
// ele é `position:fixed` e `aria-hidden`, então não entra no flex nem na árvore de acessibilidade.
//
// E `nativeButton` é o par obrigatório do `render` acima — medido em 11/08/2026. O `Radio.Root`
// declara `nativeButton = false` por padrão (é o valor certo para o `<span>` que ele renderiza
// sozinho), e o `useButton` do motor CONFERE isso no DOM montado, dentro de um efeito: sem esta
// palavra sai um `console.error` do Base UI a cada montagem de CLIENTE. Não no catálogo, que é
// HTML estático — no console de quem INSTALA.
// O DOM foi medido nos dois lados antes da troca. O `role="button"` que a mensagem ameaça NÃO
// aparecia (o `role="radio"` do motor já vencia a fusão); o que muda de fato é o `<input>`
// escondido perder o `id`, que migra para este botão — e migra para o lado certo, porque o botão
// é o rádio e o input é `aria-hidden`. Comportamento idêntico: Enter não seleciona, Espaço
// seleciona, a seta move E seleciona. O que sai é a camada sintética que o motor punha por cima
// da ativação nativa do `<button>`.
//
// `fullWidth` (A5, 06/10/2026): o mesmo nome e o mesmo desenho do nativo — o controle ocupa a
// linha e cada segmento CRESCE a partir do rótulo, sem encolher abaixo dele. É como o grupo
// de botões alternáveis da referência ocupa a linha.
export function SegmentedControl({items,value,onChange,label,fullWidth}:{items:Array<{value:string;label:ReactNode}>;value:string;onChange:(v:string)=>void;label?:string;fullWidth?:boolean}){
const s=useAureaStrings();
return <BaseRadioGroup className={fullWidth?"segmented segmented-full":"segmented"} aria-label={label??s.optionsLabel} value={value} onValueChange={v=>onChange(String(v))}>
{items.map(i=><BaseRadio.Root key={i.value} value={i.value} className={i.value===value?"active":undefined} nativeButton render={<button type="button"/>}>{i.label}</BaseRadio.Root>)}
</BaseRadioGroup>}


// Combobox básico: input + lista filtrável de item único. O filtro é do Base UI
// (via items + itemToStringLabel). Multi-seleção/async ficam para a Fase 4.
export interface ComboboxOption{value:string;label:string}
// ── AGRUPAMENTO, REPORTADO no merge de 28/08/2026 ───────────────────────────────────────────
// O `Combobox` de seleção única agrupava na outra linhagem e voltou a não agrupar quando este
// arquivo entrou inteiro da `main`. A perda foi silenciosa pelo mesmo motivo do `Tabs` e do
// `NumberField`: a conferência comparou NOMES, e `Combobox` existe nos dois lados.
//
// COMO O MOTOR RECEBE ITEM AGRUPADO — medido lendo o `AriaCombobox` do Base UI 1.6.0, não
// presumido. É a MESMA prop `items`: o tipo é `readonly any[] | readonly Group<any>[]`, e `Group`
// é só `{items: Item[]}` com o resto das chaves livre, então `{label, items}` serve sem
// adaptador. O motor achata para resolver rótulo e seleção, e filtra DENTRO de cada grupo,
// descartando o que ficou vazio e devolvendo `{...grupo, items: filtrados}`. Por isso o que chega
// ao `List` já vem filtrado, e no caso agrupado o que chega é o GRUPO.
//
// O PREDICADO É O DO MOTOR, letra por letra. Havia dois — o dele, que decide o FILTRO, e um
// local, que decidia a MARCAÇÃO — e bastava discordarem para a lista sair com marcação de grupo
// e conteúdo plano. O `typeof === "object" && != null` não é zelo: `"items" in x` LANÇA
// TypeError em primitivo, onde o do motor devolve `false`.
export interface ComboboxOptGroup{label:string;items:ComboboxOption[]}
const ehAgrupado=(items:ComboboxOption[]|ComboboxOptGroup[]):items is ComboboxOptGroup[]=>
  items!=null&&items.length>0&&typeof items[0]==="object"&&items[0]!=null&&"items" in items[0];
// UM renderizador e UMA lista para os dois combobox: duas cópias da mesma árvore é como a
// segunda fica para trás na mudança seguinte.
// A COLUNA DO CHECK EXISTE SEMPRE (A-01, 23/09/2026). O `ItemIndicator` do Base UI não põe
// NADA no DOM quando o item não está escolhido; com um filho só, o texto caía na primeira coluna
// da grade — a de 16px — e quebrava em duas ou três linhas. O invólucro fica montado sempre, como
// o `.menu-mark` do `Menu` já faz, e o indicador aparece e some DENTRO dele.
const renderComboboxItem=(item:ComboboxOption)=><BaseCombobox.Item key={item.value} value={item} className="menu-item combobox-item">
  <span className="combobox-check" aria-hidden="true"><BaseCombobox.ItemIndicator><Icon name="check" size="sm"/></BaseCombobox.ItemIndicator></span>
  <span>{item.label}</span>
</BaseCombobox.Item>;
const listaCombobox=(items:ComboboxOption[]|ComboboxOptGroup[])=><BaseCombobox.List>
  {ehAgrupado(items)
    ?(group:ComboboxOptGroup)=><BaseCombobox.Group key={group.label} items={group.items} className="combobox-section">
        <BaseCombobox.GroupLabel className="combobox-group-label">{group.label}</BaseCombobox.GroupLabel>
        <BaseCombobox.Collection>{renderComboboxItem}</BaseCombobox.Collection>
      </BaseCombobox.Group>
    :renderComboboxItem}
</BaseCombobox.List>;
// `aria-describedby` e `aria-invalid` são explícitos e não caem em `...props` por acidente: este
// componente destrincha uma lista fixa, então o que não está aqui é DESCARTADO em silêncio — e era
// exatamente isso que fazia o `Field` não alcançar o input do Combobox (AUD-0001, 12/08/2026).
// Vão para o `Input`, que é o nó focal: no contêiner eles não seriam anunciados.
// `disabled` vai na ROOT, não no Input: `Input`, `Trigger`, `Clear` e `ChipRemove` leem todos o
// mesmo `selectors.disabled` do store do Base UI (@base-ui/react 1.6.0). Desabilitar o Input
// deixaria os dois botões vivos — abrir a lista de um campo desabilitado é o defeito, não o texto.
export function Combobox({items,value,onValueChange,placeholder,label,empty,disabled,size,id,className,"aria-describedby":descrito,"aria-invalid":invalido,"aria-label":rotulo}:{items:ComboboxOption[]|ComboboxOptGroup[];empty?:ReactNode;value?:ComboboxOption|null;onValueChange?:(v:ComboboxOption|null)=>void;placeholder?:string;label?:ReactNode;disabled?:boolean;size?:Responsive<FieldSize>;id?:string;className?:string;"aria-describedby"?:string;"aria-invalid"?:boolean|"true"|"false";
  /** C-01 (24/09/2026): nome para o leitor de tela quando não há `label` visível — um filtro numa
   *  grade sem rótulos. Vai direto para o campo. Com `label`, prefira o `label`. */
  "aria-label"?:string}){
  const s=useAureaStrings();
  const portal=usePortalContainer();
  const autoId=React.useId();
  const inputId=id??autoId;
  return <BaseCombobox.Root items={items} value={value} onValueChange={soOValor(onValueChange)} disabled={disabled} itemToStringLabel={(i:ComboboxOption)=>i.label}>
    <div className={cx("field",className)}>
      {/* <label> nativo, não Combobox.Label: este associa ao TRIGGER, deixando o
          input sem nome acessível e sobrescrevendo o aria-label do botão. */}
      {label&&<label className="label" htmlFor={inputId}>{label}</label>}
      <BaseCombobox.InputGroup className="combobox-group">
        <BaseCombobox.Input id={inputId} placeholder={placeholder} className={cx("input",peleDoEixo("input",size))} aria-describedby={descrito} aria-invalid={invalido} aria-label={rotulo}/>
        <span className="combobox-actions">
          <BaseCombobox.Clear render={<IconButton variant="ghost" size="sm" icon="x" label={s.comboboxClear}/>}/>
          <BaseCombobox.Trigger render={<IconButton variant="ghost" size="sm" icon="caret-down" label={s.comboboxOpen}/>}/>
        </span>
      </BaseCombobox.InputGroup>
    </div>
    <BaseCombobox.Portal container={portal}><BaseCombobox.Positioner sideOffset={6}>
      <BaseCombobox.Popup className="menu combobox-popup">
        <BaseCombobox.Empty className="combobox-empty">{empty??s.comboboxEmpty}</BaseCombobox.Empty>
        {listaCombobox(items)}
      </BaseCombobox.Popup>
    </BaseCombobox.Positioner></BaseCombobox.Portal>
  </BaseCombobox.Root>;
}
// MultiCombobox (Fase 4): multi-seleção com chips, opções agrupadas e filtro
// externo (async). Reusa .combobox-popup/-item/-empty do básico.
// - Agrupar: passar ComboboxOptGroup[] em items (label + items). Detecção pela
//   presença de .items no 1º elemento — não misturar plano com agrupado.
// - Async: passar onInputChange; o filtro interno do Base UI desliga
//   (filter={null}) e o consumidor troca os items conforme a busca retorna.
//   loading só troca o texto do vazio para não piscar "Nenhum resultado".

export function MultiCombobox({items,value,onValueChange,onInputChange,loading,placeholder,label,empty,disabled,id,className,"aria-describedby":descrito,"aria-invalid":invalido,"aria-label":rotulo}:{items:ComboboxOption[]|ComboboxOptGroup[];empty?:ReactNode;value?:ComboboxOption[];onValueChange?:(v:ComboboxOption[])=>void;onInputChange?:(query:string)=>void;loading?:boolean;placeholder?:string;label?:ReactNode;disabled?:boolean;id?:string;className?:string;"aria-describedby"?:string;"aria-invalid"?:boolean|"true"|"false";
  /** C-01 (24/09/2026): nome para o leitor de tela quando não há `label` visível — um filtro numa
   *  grade sem rótulos. Vai direto para o campo. Com `label`, prefira o `label`. */
  "aria-label"?:string}){
  const s=useAureaStrings();
  const portal=usePortalContainer();
  const autoId=React.useId();
  const inputId=id??autoId;
  return <BaseCombobox.Root multiple items={items} value={value} onValueChange={soOValor(onValueChange)} disabled={disabled} itemToStringLabel={(i:ComboboxOption)=>i.label} filter={onInputChange?null:undefined} onInputValueChange={onInputChange?((v:string,d:{reason:string})=>{if(d.reason!=="item-press")onInputChange(v)}):undefined}>
    <div className={cx("field",className)}>
      {label&&<label className="label" htmlFor={inputId}>{label}</label>}
      <BaseCombobox.InputGroup className="combobox-multi">
        <BaseCombobox.Chips className="combobox-chips">
          <BaseCombobox.Value>
            {(selected:ComboboxOption[])=><>
              {selected.map((item:ComboboxOption)=><BaseCombobox.Chip key={item.value} className="combobox-chip">
                {item.label}
                <BaseCombobox.ChipRemove className="combobox-chip-remove" aria-label={`${s.comboboxRemove} ${item.label}`}><Icon name="x" size="sm"/></BaseCombobox.ChipRemove>
              </BaseCombobox.Chip>)}
              <BaseCombobox.Input id={inputId} placeholder={selected.length?undefined:placeholder} className="combobox-chip-input" aria-describedby={descrito} aria-invalid={invalido} aria-label={rotulo}/>
            </>}
          </BaseCombobox.Value>
        </BaseCombobox.Chips>
        <span className="combobox-actions">
          <BaseCombobox.Clear render={<IconButton variant="ghost" size="sm" icon="x" label={s.comboboxClear}/>}/>
          <BaseCombobox.Trigger render={<IconButton variant="ghost" size="sm" icon="caret-down" label={s.comboboxOpen}/>}/>
        </span>
      </BaseCombobox.InputGroup>
    </div>
    <BaseCombobox.Portal container={portal}><BaseCombobox.Positioner sideOffset={6}>
      <BaseCombobox.Popup className="menu combobox-popup">
        <BaseCombobox.Empty className="combobox-empty">{loading?s.comboboxLoading:(empty??s.comboboxEmpty)}</BaseCombobox.Empty>
        {listaCombobox(items)}
      </BaseCombobox.Popup>
    </BaseCombobox.Positioner></BaseCombobox.Portal>
  </BaseCombobox.Root>;
}

// ── Select (A-02 e B-08, 24/09/2026) ────────────────────────────────────────────────────────
// A LISTA É DA AUREA, NÃO DO SISTEMA OPERACIONAL. Até a 0.8.9 o `Select` era o `<select>` nativo, e
// a lista que ele abre é desenhada pelo navegador: canto reto, azul do Windows, fonte do sistema —
// CSS de autor não a alcança. Sem outra escolha fechada com a pele da casa, um app real pôs cinco
// resoluções e seis ordenações atrás do `Combobox`, que é sempre um campo de BUSCA. O Victor viu a
// busca numa lista de cinco e decidiu: o `Select` passa a ser o padrão Aurea, sem componente novo.
//
// A FORMA DE ESCREVER NÃO MUDA. `<option>` e `<optgroup>` dentro, `value`/`defaultValue` em string,
// `onChange` lendo `e.target.value`, `name`, `disabled`, `size` — o que funcionava continua
// funcionando. Por baixo é o `Select` do Base UI, a MESMA base do `Combobox`: o gatilho veste a pele
// `.select` (a cápsula e a seta de sempre) e a lista veste a pele da lista do `Combobox` (painel,
// raio de 18px, coluna do check). `items` é a forma nova, para quem monta a lista a partir de dados.
//
// O QUE MUDOU DE VERDADE, e é por isso que está escrito: o nó é um <button> (papel `combobox`), não
// um <select>. O `ref` aponta para ele. E sem `value`, `defaultValue` nem `placeholder` o primeiro
// item vem escolhido — é o que o <select> nativo fazia, e quem dependia disso não percebe a troca.
// `alignItemWithTrigger={false}`: o padrão do motor abre a lista POR CIMA do gatilho (jeito do macOS);
// nesta casa as listas abrem embaixo, e duas maneiras de abrir o mesmo desenho seria o defeito.
export interface SelectOption{value:string;label:ReactNode;disabled?:boolean}
export interface SelectOptGroup{label:ReactNode;items:SelectOption[]}
type SelectChange={target:{value:string;name?:string};currentTarget:{value:string;name?:string}};
export interface SelectProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>,"value"|"defaultValue"|"onChange"|"name">,RefAttributes<HTMLButtonElement>{
  items?:SelectOption[]|SelectOptGroup[];value?:string;defaultValue?:string;onChange?:(e:SelectChange)=>void;
  onValueChange?:(value:string)=>void;placeholder?:ReactNode;name?:string;required?:boolean;size?:Responsive<FieldSize>}
const ehGrupoDoSelect=(i:SelectOption|SelectOptGroup):i is SelectOptGroup=>typeof i==="object"&&i!=null&&"items" in i;
// Lê `<option>`/`<optgroup>` como o navegador lê: `value` ausente vale o TEXTO da opção.
function opcoesDosFilhos(children:ReactNode):(SelectOption|SelectOptGroup)[]{
  const lista:(SelectOption|SelectOptGroup)[]=[];
  React.Children.forEach(children,filho=>{
    if(!React.isValidElement(filho))return;
    const pr=filho.props as {value?:unknown;children?:ReactNode;disabled?:boolean;label?:ReactNode};
    if(filho.type==="option")lista.push({value:String(pr.value??React.Children.toArray(pr.children).join("")),label:pr.children,disabled:pr.disabled});
    else if(filho.type==="optgroup")lista.push({label:pr.label,items:opcoesDosFilhos(pr.children) as SelectOption[]});
    else if(filho.type===React.Fragment)lista.push(...opcoesDosFilhos(pr.children));
  });
  return lista;
}
const itemDoSelect=(o:SelectOption)=><BaseSelect.Item key={o.value} value={o.value} disabled={o.disabled} className="menu-item combobox-item">
  <span className="combobox-check" aria-hidden="true"><BaseSelect.ItemIndicator><Icon name="check" size="sm"/></BaseSelect.ItemIndicator></span>
  <BaseSelect.ItemText>{o.label}</BaseSelect.ItemText>
</BaseSelect.Item>;
export const Select=forwardRef<HTMLButtonElement,SelectProps>(function Select({items,children,value,defaultValue,onChange,onValueChange,placeholder,name,required,disabled,size,className,...props},ref){
  const portal=usePortalContainer();
  const lista=items??opcoesDosFilhos(children);
  const planas=lista.flatMap(i=>ehGrupoDoSelect(i)?i.items:[i]);
  const inicial=value===undefined&&defaultValue===undefined&&placeholder===undefined?planas.find(o=>!o.disabled)?.value:defaultValue;
  const mudar=(v:string|null)=>{
    const valor=v??"";
    onValueChange?.(valor);
    onChange?.({target:{value:valor,name},currentTarget:{value:valor,name}});
  };
  return <BaseSelect.Root items={planas.map(o=>({value:o.value,label:o.label}))} {...(value!==undefined?{value}:{defaultValue:inicial})}
    onValueChange={v=>mudar(v as string|null)} name={name} required={required} disabled={disabled}>
    <BaseSelect.Trigger ref={ref} className={cx("select","select-trigger",peleDoEixo("select",size),className)} {...props}>
      <BaseSelect.Value placeholder={placeholder} className="select-value"/>
    </BaseSelect.Trigger>
    <BaseSelect.Portal container={portal}><BaseSelect.Positioner sideOffset={6} alignItemWithTrigger={false}>
      <BaseSelect.Popup className="menu combobox-popup">
        <BaseSelect.List>
          {lista.map((i,n)=>ehGrupoDoSelect(i)
            ?<BaseSelect.Group key={n} className="combobox-section">
                <BaseSelect.GroupLabel className="combobox-group-label">{i.label}</BaseSelect.GroupLabel>
                {i.items.map(itemDoSelect)}
              </BaseSelect.Group>
            :itemDoSelect(i))}
        </BaseSelect.List>
      </BaseSelect.Popup>
    </BaseSelect.Positioner></BaseSelect.Portal>
  </BaseSelect.Root>;
});

// ── BlockEditor (PLANO-1.0, item N1) ─────────────────────────────────────────────────────────
// A DECISÃO QUE O ITEM MANDAVA TOMAR ANTES DE ESCREVER está na
// [ADR-0025](../../../decisions/0025-editor-por-blocos-sem-motor.md): a Aurea entrega a PELE DOS
// BLOCOS e o motor de texto rico fica com o consumidor. Não é preferência — é o que a medição
// devolveu, em quatro partes:
//
// 1. A REFERÊNCIA NÃO É O MESMO COMPONENTE, e essa é a primeira pergunta do `BUILDING.md` §2
//    passo 2. O `editor` de uma das referências (na pasta `Referencia/`) tem 39 exports e é feito sobre um
//    motor de texto rico de terceiro, com 17 dependências no `package.json` — e é um editor de DOCUMENTO ÚNICO:
//    negrito, itálico, tabelas, menu-bolha. Ele não tem lista de blocos, não tem reordenação e
//    não tem bloco de imagem com legenda. Anatomia de bloco, ali, não existe para copiar.
// 2. EMBRULHAR O MOTOR CUSTARIA A TODO CONSUMIDOR. É a conta que o `Chart` e o `Calendar` já
//    pagaram de propósito em subpath próprio — mas lá o motor ERA o componente. Aqui o que o
//    item descreve ("texto, imagem com legenda") é moldura, e moldura não justifica um motor de texto rico
//    no pacote de quem só quer um artigo.
// 3. O QUE FALTAVA JÁ ESTAVA QUASE TODO AQUI, medido no passo 1: `Prose` (L5) desenha a saída,
//    `Image` (L4) é o bloco de imagem, `Textarea` e `Field` são a entrada, `Toolbar` é a barra.
//    O buraco era a MOLDURA — ordem, alça, remoção, e o lugar onde o editor do consumidor entra.
// 4. SEGURANÇA, e esta é a razão que fecha a porta. Editor que é dono do texto rico é dono da
//    colagem, e o navegador NÃO sanitiza HTML colado: quem cola vira XSS. A pesquisa de 15/08
//    confirma que os motores de texto rico de mercado tratam o `contenteditable` como ALVO de renderização e
//    nunca como fonte da verdade, justamente por isso. A Aurea não pode decidir o que é seguro
//    renderizar no domínio do consumidor — é a mesma frase que a ficha do `Prose` já diz em voz
//    alta. Entregando a moldura, a superfície de colagem continua com quem tem o contexto.
//
// O QUE ESTE COMPONENTE NÃO TEM, declarado: nada de negrito/itálico (é do motor do consumidor),
// nada de menu-bolha, nada de barra de formatação, e nada de `onInsert`. O último é YAGNI
// medido, não esquecimento: acrescentar bloco é APENDAR na coleção do consumidor, e a
// reordenação daqui leva o novo bloco a qualquer posição. Uma costura de inserção entre blocos
// seria uma segunda forma de fazer a mesma coisa.
//
// CONTROLADO, como a `SortableList`, a `Gallery` e as `Tabs`: `blocks` e os dois callbacks. O
// componente não guarda ordem nem conteúdo.
export interface EditorBlock{id:string;kind?:ReactNode;children:ReactNode}
export interface BlockEditorProps extends Omit<HTMLAttributes<HTMLOListElement>,"onReorder">,RefAttributes<HTMLOListElement>{
  blocks:EditorBlock[];onReorder:(from:number,to:number)=>void;onRemove?:(index:number)=>void;label?:string;
}
export function BlockEditor({blocks,onReorder,onRemove,label,className,...props}:BlockEditorProps){
  const s=useAureaStrings();
  const bid=React.useId();
  const n=blocks.length;
  // O protocolo de arrasto acessível é o MESMO da `SortableList` — extraído para o `internal` em
  // vez de copiado, porque duas cópias garantem que a próxima correção entre em uma só.
  const r=useReorder<HTMLOListElement>({count:n,order:blocks,onReorder,
    rowSelector:".block-item",handleSelector:".block-handle"});
  return <>
    <ol ref={r.ref} className={cx("block-editor",className)} aria-label={label??s.blockEditorLabel} {...props}>
      {blocks.map((b,i)=>{
        const kid=`${bid}k${i}`,hid=`${bid}h${i}`,rid=`${bid}r${i}`;
        return <li key={b.id} className="block-item" data-grabbed={r.pego===i||undefined}>
          {/* O conteúdo do bloco é do consumidor e é conteúdo de FLUXO — figura, legenda, campo.
              Por isso ele mora num `<div>` dentro do `<li>`, e não no `<span>` que a
              `SortableList` usa: `<span>` só aceita conteúdo de frase, e uma `<figure>` ali
              dentro é marcação inválida que o navegador reescreve. Foi o motivo medido de este
              componente existir em vez de virar uma prop da `SortableList`. */}
          <div className="block-rail">
            {/* Os dois botões são nomeados pelo próprio texto MAIS o rótulo do bloco: sai
                "Reordenar, Imagem 2 de 5, botão", sem o consumidor mandar o texto duas vezes. */}
            <button type="button" id={hid} className="block-handle" aria-labelledby={`${hid} ${kid}`}
              aria-describedby={`${bid}ajuda`} aria-pressed={r.pego===i}
              onKeyDown={e=>r.teclado(e,i)} onPointerDown={e=>r.ponteiroBaixo(e,i)}
              onPointerMove={r.ponteiroMove} onPointerUp={r.ponteiroSolta} onPointerCancel={r.ponteiroSolta}>
              <Icon name="dots-six"/><span className="sr-only">{s.sortableHandle}</span>
            </button>
            {onRemove&&<button type="button" id={rid} className="block-remove" aria-labelledby={`${rid} ${kid}`}
              onClick={()=>onRemove(i)}>
              <Icon name="trash"/><span className="sr-only">{s.blockRemove}</span>
            </button>}
          </div>
          <div className="block-body">{b.children}</div>
          {/* A posição vai no NOME dos controles e não numa região viva: aqui ela é permanente
              ("Imagem 2 de 5"), enquanto a região viva abaixo é o que MUDOU agora. */}
          <span id={kid} className="sr-only">{b.kind??s.blockLabel} {i+1} {s.positionOf} {n}</span>
        </li>;
      })}
    </ol>
    <span id={`${bid}ajuda`} className="sr-only">{s.sortableHelp}</span>
    <div role="status" aria-live="polite" className="sr-only">{r.aviso}</div>
  </>;
}

// ── PORTADO da linhagem da ATIVIDADE-2 no merge de 28/08/2026 ───────────────────────────────
// Não existia na `main`. Fica no módulo de CLIENTE porque guarda estado (`useState` do
// mostrar/ocultar); a moldura (`InputGroup`) vem do `markup`, que é servidor.
// CAMPO DE SENHA com mostrar/ocultar. Duas das nove referências têm (com nomes diferentes
// em cada uma) e as duas convergem na anatomia: o campo, um botão ao FIM dele,
// `aria-controls` apontando para o campo, e o rótulo do botão mudando com o estado.
//
// O rótulo que muda é o anúncio — não há `aria-pressed`. Com os dois, o leitor de tela diz duas
// vezes a mesma coisa ("Mostrar senha, não pressionado"), e nenhuma das duas referências faz isso.
//
// O `type` é do componente e não passa: um campo de senha que aceitasse `type` deixaria de ser um.
export interface PasswordFieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>,"size"|"type">,RefAttributes<HTMLInputElement>{size?:Responsive<FieldSize>;defaultVisible?:boolean}
export function PasswordField({className,size,defaultVisible=false,id,...props}:PasswordFieldProps){
  const s=useAureaStrings();
  const [visivel,setVisivel]=React.useState(defaultVisible);
  const auto=React.useId();
  const campoId=id??auto;
  return <InputGroup className={className}>
    {/* O DEGRAU SAI COMO CLASSE, e não pela prop `size` do `Input`. Hoje os dois dariam no
        mesmo — o `size` do `Input` é o degrau do sistema desde 28/08/2026 —, mas emitir a
        classe aqui deixa o caminho explícito e sobrevive a `Input` mudar de ideia. É a mesma
        classe que o core pinta (`peleDoEixo`). */}
    <Input id={campoId} type={visivel?"text":"password"} className={peleDoEixo("input",size)||undefined} autoComplete="current-password" {...props}/>
    <InputGroupAddon side="end" layout="inline">
      <IconButton variant="ghost" size={valorBase(size)==="lg"?"md":"sm"}
        icon={visivel?"eye-slash":"eye"} label={visivel?s.passwordHide:s.passwordShow}
        aria-controls={campoId} onClick={()=>setVisivel(v=>!v)}/>
    </InputGroupAddon>
  </InputGroup>;
}
