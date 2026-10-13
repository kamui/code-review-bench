// components/item/ItemForm.tsx (versione con danno selezionabile)
'use client';

import { useState, useEffect } from 'react';
import { useRouter } from 'next/navigation';
import { toast } from 'sonner';
import { useCreateItem, useUpdateItem, useDeleteItem } from '@/hooks/mutations/useItemMutations';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { 
  Select, 
  SelectContent, 
  SelectItem, 
  SelectTrigger, 
  SelectValue 
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import { 
  Save, 
  X, 
  Trash2,
  Sword,
  Shield,
  FlaskConical,
  Wrench,
  Coins,
  ArrowUpDown,
  Package,
  Plus,
  Trash,
  Dice6
} from 'lucide-react';
import { cn } from '@/lib/utils';
import AncientContainer from '@/components/custom/AncientContainer';
import { AntiqueButton } from '@/components/custom/AntiqueButton';
import {
  CheckboxField,
  DamageTypeSelect,
  DiceSelect,
  ItemPropertyHeader,
  MagicBonusField,
  NumberField,
} from '@/components/items/ItemFormFields';
import {
  CURRENCY_OPTIONS,
  ITEM_TYPE_OPTIONS,
  RARITY_OPTIONS,
  VERSATILE_DICE_OPTIONS,
  WEAPON_PROPERTY_OPTIONS,
} from '@/components/items/itemOptions';
import type { 
  CreateItemDTO, 
  ItemType, 
  Rarity, 
  CurrencyType,
  WeaponProperties,
  ItemProperties,
  ArmorProperties,
  ConsumableProperties,
  AmmunitionProperties,
  ToolProperties,
  GearProperties
} from '@/types/item';
import { Separator } from '@base-ui/react';

type ItemFormProps = {
  mode?: 'create' | 'edit' | 'view';
  itemId?: number;
  initialData?: Partial<CreateItemDTO>;
  onSubmit?: (data: CreateItemDTO) => Promise<void>;
  onDelete?: () => Promise<void>;
  isLoading?: boolean;
  title?: string;
  subtitle?: string;
};

export default function ItemForm({
  mode = 'create',
  itemId,
  initialData = {},
  onSubmit,
  onDelete,
  isLoading: isLoadingProp = false,
  title = mode === 'edit' ? 'Modifica Oggetto' : 'Nuovo Oggetto',
  subtitle = mode === 'edit' ? 'Modifica i dettagli dell\'oggetto' : 'Crea o modifica un oggetto per il gioco',
}: ItemFormProps) {
  const router = useRouter();
  const createItem = useCreateItem();
  const updateItem = useUpdateItem();
  const deleteItem = useDeleteItem();

  const isLoading = isLoadingProp || createItem.isPending || updateItem.isPending || deleteItem.isPending;
  const [selectedType, setSelectedType] = useState<ItemType>(initialData.type || 'gear');
  const [formData, setFormData] = useState<CreateItemDTO>({
    name: '',
    type: 'gear',
    weight: 0,
    value: 0,
    currency: 'po',
    rarity: 'common',
    requires_attunement: false,
    category: null,
    description: null,
    properties: null,
    ...initialData,
  });

  const [errors, setErrors] = useState<Record<string, string>>({});
  const isView = mode === 'view';

  useEffect(() => {
    setFormData(prev => ({ ...prev, type: selectedType }));
  }, [selectedType]);

  // Gestisce le proprietà in modo strutturato
  const updateProperties = (updates: Partial<ItemProperties>) => {
    setFormData(prev => {
      const newProps = ({ ...(prev.properties ?? {}), ...updates }) as ItemProperties;
      return { ...prev, properties: newProps };
    });
  };

  const validate = (): boolean => {
    const newErrors: Record<string, string> = {};
    
    if (!formData.name?.trim()) {
      newErrors.name = 'Il nome è obbligatorio';
    }
    
    if (formData.weight < 0) {
      newErrors.weight = 'Il peso non può essere negativo';
    }
    
    if (formData.value < 0) {
      newErrors.value = 'Il valore non può essere negativo';
    }
    
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!validate()) return;

    if (onSubmit) {
      await onSubmit(formData);
      return;
    }

    if (mode === 'edit' && itemId) {
      updateItem.mutate(
        { id: itemId, data: { ...formData, id: itemId } },
        {
          onSuccess: () => {
            toast.success('Oggetto aggiornato!');
            router.push('/admin/items');
          },
          onError: (err) => toast.error(err instanceof Error ? err.message : 'Errore aggiornamento'),
        }
      );
    } else {
      createItem.mutate(formData, {
        onSuccess: () => {
          toast.success('Oggetto creato!');
          router.push('/admin/items');
        },
        onError: (err) => toast.error(err instanceof Error ? err.message : 'Errore creazione'),
      });
    }
  };

  const handleDelete = async () => {
    if (onDelete) {
      await onDelete();
      return;
    }
    if (itemId) {
      deleteItem.mutate(itemId, {
        onSuccess: () => {
          toast.success('Oggetto eliminato!');
          router.push('/admin/items');
        },
        onError: (err) => toast.error(err instanceof Error ? err.message : 'Errore eliminazione'),
      });
    }
  };

  // ===========================================
  // RENDER DELLE PROPRIETÀ PER TIPO
  // ===========================================

  const renderWeaponProperties = () => {
    const props = formData.properties as WeaponProperties || { itemType: 'weapon' };
    const extraDamage = props.extraDamage ?? [];
    const isVersatile = props.properties?.includes('versatile');

    const setExtraDamage = (next: WeaponProperties['extraDamage']) =>
      updateProperties({ extraDamage: next });
    
    return (
        <>
        <ItemPropertyHeader icon={Sword} title="Proprietà dell'Arma" />
        <div className="pt-4">
          <div className="grid grid-cols-2 gap-4">
            {/* Dado Danno */}
            <div>
              <Label className="flex items-center gap-2">
                <Dice6 className="w-4 h-4" />
                Dado Danno
              </Label>
              <DiceSelect
                className="mt-1"
                value={props.damage}
                onChange={(damage) => updateProperties({ damage })}
              />
            </div>

            {/* Tipo Danno */}
            <div>
              <Label>Tipo Danno</Label>
              <DamageTypeSelect
                className="mt-1"
                value={props.damageType}
                onChange={(damageType) => updateProperties({ damageType })}
              />
            </div>
          </div>

          {/* Statistica per il Danno */}
          <div>
            <Label className="flex items-center gap-2">
              Statistica per il Danno
              <span className="text-xs font-normal text-gray-400">(opzionale)</span>
            </Label>
            <Select
              value={props.damageAbility ?? ''}
              onValueChange={(value) => updateProperties({ damageAbility: value ? (value as 'strength' | 'dexterity') : null })}
            >
              <SelectTrigger className="mt-1 w-full">
                <SelectValue placeholder="Nessuna (usa bonus proficiency)">
                  {props.damageAbility === 'strength' ? 'Forza (FOR)' :
                   props.damageAbility === 'dexterity' ? 'Destrezza (DES)' :
                   'Nessuna (usa bonus proficiency)'}
                </SelectValue>
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="" label="Nessuna">
                  <span className="text-gray-500">Nessuna (usa bonus proficiency)</span>
                </SelectItem>
                <SelectItem value="strength" label="Forza (FOR)">
                  Forza (FOR)
                </SelectItem>
                <SelectItem value="dexterity" label="Destrezza (DES)">
                  Destrezza (DES)
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Danni Aggiuntivi */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <Label className="flex items-center gap-2">
                Danni Aggiuntivi
                <span className="text-xs font-normal text-gray-400">(es. +1d6 fuoco magico)</span>
              </Label>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="flex items-center gap-1 text-xs"
                onClick={() => {
                  const current = props.extraDamage || [];
                  setExtraDamage([...current, { dice: '1d6', type: 'fuoco' }]);
                }}
              >
                <Plus className="w-3 h-3" />
                Aggiungi
              </Button>
            </div>
            {extraDamage.length === 0 ? (
              <p className="text-xs text-gray-400 py-1">Nessun danno aggiuntivo</p>
            ) : (
              <div className="space-y-2">
                {extraDamage.map((entry, idx) => (
                  <div key={idx} className="flex items-center gap-2">
                    <DiceSelect
                      className="flex-1"
                      value={entry.dice}
                      placeholder="Dado"
                      onChange={(dice) =>
                        setExtraDamage(extraDamage.map((e, i) => (i === idx ? { ...e, dice } : e)))
                      }
                    />
                    <DamageTypeSelect
                      className="flex-1"
                      value={entry.type}
                      placeholder="Tipo"
                      onChange={(type) =>
                        setExtraDamage(extraDamage.map((e, i) => (i === idx ? { ...e, type } : e)))
                      }
                    />
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      className="text-red-400 hover:text-red-600 hover:bg-red-50 flex-shrink-0"
                      onClick={() => setExtraDamage(extraDamage.filter((_, i) => i !== idx))}
                    >
                      <Trash className="w-4 h-4" />
                    </Button>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Danno Versatile (condizionale) */}
          {isVersatile && (
            <div className="grid grid-cols-2 gap-4">
              <div>
                <Label>Danno Versatile</Label>
                <DiceSelect
                  className="mt-1"
                  options={VERSATILE_DICE_OPTIONS}
                  value={props.versatileDamage}
                  onChange={(versatileDamage) => updateProperties({ versatileDamage })}
                />
              </div>
              <div className="text-xs text-gray-500 flex items-end pb-2">
                Quando usata a due mani
              </div>
            </div>
          )}

          {/* Proprietà arma (checkboxes con descrizione) */}
          <div>
            <Label>Proprietà</Label>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2 mt-1">
              {WEAPON_PROPERTY_OPTIONS.map(prop => (
                <CheckboxField
                  key={prop.value}
                  label={prop.label}
                  description={prop.description}
                  checked={props.properties?.includes(prop.value) ?? false}
                  onCheckedChange={(checked) => {
                    const current = props.properties || [];
                    updateProperties({
                      properties: checked
                        ? [...current, prop.value]
                        : current.filter(p => p !== prop.value),
                    });
                  }}
                />
              ))}
            </div>
          </div>

          {/* Gittata */}
          <div className="grid grid-cols-2 gap-4">
            <NumberField
              label="Gittata Normale (m)"
              value={props.range?.normal}
              emptyValue={props.range?.normal ?? 0}
              onChange={(normal) => updateProperties({ range: { normal, long: props.range?.long } })}
              placeholder="es. 6"
            />
            <NumberField
              label="Gittata Lunga (m)"
              value={props.range?.long}
              onChange={(long) => updateProperties({ range: { normal: props.range?.normal ?? 0, long } })}
              placeholder="es. 18"
            />
          </div>

          {/* Bonus magico */}
          <MagicBonusField
            value={props.magicBonus}
            onChange={(magicBonus) => updateProperties({ magicBonus })}
          />
        </div>
      </>
    );
  };

  const renderArmorProperties = () => {
    const props = formData.properties as ArmorProperties || { itemType: 'armor' };
    
    return (
      <>
        <ItemPropertyHeader icon={Shield} title="Proprietà dell'Armatura" />
        <div className="pt-4">

          <div className="grid grid-cols-2 gap-4">
            <NumberField
              label="Classe Armatura (CA)"
              value={props.armorClass}
              onChange={(armorClass) => updateProperties({ armorClass })}
            />
            <div>
              <Label>Tipo Armatura</Label>
              <Select
                value={props.armorType ?? ''}
                onValueChange={(value) => updateProperties({ armorType: value as ArmorProperties['armorType'] })}
              >
                <SelectTrigger className="mt-1">
                  <SelectValue placeholder="Seleziona tipo" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="light" label="Leggera">Leggera</SelectItem>
                  <SelectItem value="medium" label="Media">Media</SelectItem>
                  <SelectItem value="heavy" label="Pesante">Pesante</SelectItem>
                  <SelectItem value="shield" label="Scudo">Scudo</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="flex flex-wrap gap-4">
            <CheckboxField
              label="Aggiunge modificatore Destrezza"
              checked={props.addsDexModifier || false}
              onCheckedChange={(addsDexModifier) => updateProperties({ addsDexModifier })}
            />

            {props.armorType === 'medium' && (
              <div className="flex items-center gap-2">
                <Label>Max Bonus Destrezza</Label>
                <Input
                  type="number"
                  value={props.maxDexBonus || ''}
                  onChange={(e) => updateProperties({ maxDexBonus: parseInt(e.target.value) || 0 })}
                  className="w-20"
                  placeholder="2"
                />
              </div>
            )}
          </div>

          <div className="flex flex-wrap gap-4">
            <CheckboxField
              label="Svantaggio su Furtività"
              checked={props.stealthDisadvantage || false}
              onCheckedChange={(stealthDisadvantage) => updateProperties({ stealthDisadvantage })}
            />

            <div className="flex items-center gap-2">
              <Label>Richiede Forza</Label>
              <Input
                type="number"
                value={props.strengthRequirement || ''}
                onChange={(e) => updateProperties({ strengthRequirement: parseInt(e.target.value) || 0 })}
                className="w-20"
                placeholder="13"
              />
            </div>
          </div>

          {/* Bonus magico */}
          <MagicBonusField
            value={props.magicBonus}
            onChange={(magicBonus) => updateProperties({ magicBonus })}
          />
        </div>
      </>
    );
  };

  const renderConsumableProperties = () => {
    const props = formData.properties as ConsumableProperties || { itemType: 'consumable' };
    
    return (
      <>
        <ItemPropertyHeader icon={FlaskConical} title="Proprietà del Consumabile" />
        <div className="pt-4">
          <div>
            <Label>Effetto</Label>
            <Textarea
              value={props.effect || ''}
              onChange={(e) => updateProperties({ effect: e.target.value })}
              placeholder="es. Recupera 2d4+2 PF, Invisibilità per 1 ora, +2 Forza per 1 ora"
              className="mt-1"
              rows={3}
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label>Durata</Label>
              <Input
                value={props.duration || ''}
                onChange={(e) => updateProperties({ duration: e.target.value })}
                placeholder="es. Istantaneo, 1 ora, 24 ore"
                className="mt-1"
              />
            </div>
            <div>
              <Label>Ricarica</Label>
              <Select
                value={props.recharge ?? ''}
                onValueChange={(value) => updateProperties({ recharge: value as string })}
              >
                <SelectTrigger className="mt-1">
                  <SelectValue placeholder="Nessuna" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="short">Riposo Breve</SelectItem>
                  <SelectItem value="long">Riposo Lungo</SelectItem>
                  <SelectItem value="dawn">Alba</SelectItem>
                  <SelectItem value="dusk">Crepuscolo</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <NumberField
            label="Utilizzi Massimi"
            value={props.usesMax}
            onChange={(usesMax) => updateProperties({ usesMax })}
            inputClassName="w-32"
            placeholder="1, 3, 10"
            hint="Lascia vuoto per utilizzo singolo"
          />
        </div>
      </>
    );
  };

  const renderAmmunitionProperties = () => {
    const props = formData.properties as AmmunitionProperties || { itemType: 'ammunition' };
    
    return (
      <>
        <ItemPropertyHeader icon={ArrowUpDown} title="Proprietà delle Munizioni" />
        <div className="pt-4">

          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label>Tipo</Label>
              <Select
                value={props.ammunitionType ?? ''}
                onValueChange={(value) => updateProperties({ ammunitionType: value as AmmunitionProperties['ammunitionType'] })}
              >
                <SelectTrigger className="mt-1">
                  <SelectValue placeholder="Seleziona tipo" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="arrow" label="Freccia">Freccia</SelectItem>
                  <SelectItem value="bolt" label="Quadrello">Quadrello</SelectItem>
                  <SelectItem value="bullet" label="Proiettile">Proiettile</SelectItem>
                  <SelectItem value="needle" label="Spillo">Spillo</SelectItem>
                </SelectContent>
              </Select>
            </div>
            <NumberField
              label="Quantità per confezione"
              value={props.quantity}
              emptyValue={1}
              onChange={(quantity) => updateProperties({ quantity })}
              placeholder="20"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <Label>Bonus Danno</Label>
              <Input
                value={props.damageBonus || ''}
                onChange={(e) => updateProperties({ damageBonus: e.target.value })}
                placeholder="es. 1d4 fuoco, +1 danno"
                className="mt-1"
              />
            </div>
            <MagicBonusField
              value={props.magicBonus}
              onChange={(magicBonus) => updateProperties({ magicBonus })}
            />
          </div>
        </div>
      </>
    );
  };

  const renderToolProperties = () => {
    const props = formData.properties as ToolProperties || { itemType: 'tool' };
    
    return (
      <>
        <ItemPropertyHeader icon={Wrench} title="Proprietà dell'Attrezzo" />
        <div className="pt-4">
          <div>
            <Label>Tipo Attrezzo</Label>
            <Input
              value={props.toolType || ''}
              onChange={(e) => updateProperties({ toolType: e.target.value })}
              placeholder="es. Attrezzi da fabbro, Attrezzi da scasso, Kit da alchimista"
              className="mt-1"
            />
          </div>
          
          <div>
            <Label>Abilità Associata</Label>
            <Input
              value={props.skill || ''}
              onChange={(e) => updateProperties({ skill: e.target.value })}
              placeholder="es. Artigianato, Scassinare, Alchimia"
              className="mt-1"
            />
          </div>
          
          <CheckboxField
            label="Richiede competenza"
            checked={!!props.proficiency}
            onCheckedChange={(proficiency) => updateProperties({ proficiency })}
          />
        </div>
      </>
    );
  };

  const renderGearProperties = () => {
    const props = formData.properties as GearProperties || { itemType: 'gear' };

    return (
      <>
        <ItemPropertyHeader icon={Package} title="Proprietà dell'Equipaggiamento" />
        <div className="pt-4">
          <NumberField
            className="mt-3"
            label="Capacità (kg)"
            value={props.capacity}
            parse="float"
            step="0.5"
            onChange={(capacity) => updateProperties({ capacity })}
            placeholder="Per contenitori (es. zaino, baule)"
            hint="Lascia vuoto se non è un contenitore"
          />
        </div>
      </>
    );
  };

  const renderCurrencyProperties = () => {
    return (
      <>
        <ItemPropertyHeader icon={Coins} title="Moneta" />
        <div className="pt-4">
          <p className="text-sm text-gray-500">
            Le monete sono gestite automaticamente nel sistema di valuta.
            Il valore viene calcolato in base alla moneta selezionata.
          </p>
        </div>
      </>
    );
  };

  const renderPropertiesByType = () => {
    switch (selectedType) {
      case 'weapon':
        return renderWeaponProperties();
      case 'armor':
        return renderArmorProperties();
      case 'consumable':
        return renderConsumableProperties();
      case 'ammunition':
        return renderAmmunitionProperties();
      case 'tool':
        return renderToolProperties();
      case 'gear':
        return renderGearProperties();
      case 'currency':
        return renderCurrencyProperties();
      default:
        return null;
    }
  };

  return (
    <AncientContainer
      title={title}
      subtitle={subtitle}
      action={(
        <div className="flex gap-2">
          {(onDelete || (mode === 'edit' && itemId)) && !isView && (
            <Button
              type="button"
              variant="outline"
              onClick={handleDelete}
              disabled={isLoading}
              className="border-red-300 text-red-600 hover:bg-red-50"
            >
              <Trash2 className="w-4 h-4 mr-2" />
              Elimina
            </Button>
          )}
          <Button type="button" variant="outline" onClick={() => router.back()}>
            <X className="w-4 h-4 mr-2" />
            {isView ? 'Chiudi' : 'Annulla'}
          </Button>
        </div>
      )}
      contentClassName="p-0"
    >

      {/* Form */}
      <form onSubmit={handleSubmit} className="p-6 space-y-6">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {/* Nome */}
          <div className="col-span-2">
            <Label htmlFor="name" className="text-gray-700 font-medium">
              Nome Oggetto <span className="text-red-500">*</span>
            </Label>
            <Input
              id="name"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              className={cn("mt-1", errors.name && "border-red-500")}
              placeholder="es. Spada lunga, Pozione di cura, Armatura di cuoio..."
            />
            {errors.name && <p className="text-red-500 text-xs mt-1">{errors.name}</p>}
          </div>

          {/* Tipo */}
          <div>
            <Label htmlFor="type" className="text-gray-700 font-medium">
              Tipo Oggetto
            </Label>
            <Select value={selectedType} onValueChange={(value) => setSelectedType(value as ItemType)}>
              <SelectTrigger className="mt-1">
                {(() => {
                  const found = ITEM_TYPE_OPTIONS.find(t => t.value === selectedType);
                  if (!found) return <SelectValue placeholder="Seleziona tipo" />;
                  const Icon = found.icon;
                  return <span className="flex items-center gap-2"><Icon className="w-4 h-4" /><span>{found.label}</span></span>;
                })()}
              </SelectTrigger>
              <SelectContent>
                {ITEM_TYPE_OPTIONS.map((type) => {
                  const Icon = type.icon;
                  return (
                    <SelectItem key={type.value} value={type.value} label={type.label}>
                      <div className="flex items-center gap-2">
                        <Icon className="w-4 h-4" />
                        <span>{type.label}</span>
                      </div>
                    </SelectItem>
                  );
                })}
              </SelectContent>
            </Select>
          </div>

          {/* Rarità */}
          <div>
            <Label htmlFor="rarity" className="text-gray-700 font-medium">
              Rarità
            </Label>
            <Select
              value={formData.rarity}
              onValueChange={(value) => setFormData({ ...formData, rarity: value as Rarity })}
            >
              <SelectTrigger className="mt-1">
                {(() => {
                  const found = RARITY_OPTIONS.find(r => r.value === formData.rarity);
                  return found
                    ? <span className={found.color}>{found.label}</span>
                    : <SelectValue placeholder="Seleziona rarità" />;
                })()}
              </SelectTrigger>
              <SelectContent>
                {RARITY_OPTIONS.map((rarity) => (
                  <SelectItem key={rarity.value} value={rarity.value}>
                    <span className={rarity.color}>{rarity.label}</span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          {/* Peso */}
          <div>
            <Label htmlFor="weight" className="text-gray-700 font-medium">
              Peso (kg)
            </Label>
            <Input
              id="weight"
              type="number"
              step="0.01"
              value={formData.weight}
              onChange={(e) => setFormData({ ...formData, weight: parseFloat(e.target.value) || 0 })}
              className="mt-1"
            />
          </div>

          {/* Valore */}
          <div>
            <Label htmlFor="value" className="text-gray-700 font-medium">
              Valore
            </Label>
            <div className="flex gap-2 mt-1">
              <Input
                id="value"
                type="number"
                step="0.01"
                value={formData.value}
                onChange={(e) => setFormData({ ...formData, value: parseFloat(e.target.value) || 0 })}
                className="flex-1"
              />
              <Select
                value={formData.currency}
                onValueChange={(value) => setFormData({ ...formData, currency: value as CurrencyType })}
              >
                <SelectTrigger className="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {CURRENCY_OPTIONS.map((curr) => (
                    <SelectItem key={curr.value} value={curr.value} label={curr.label}>
                      {curr.value.toUpperCase()}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          {/* Richiede Sintonizzazione */}
          {selectedType !== 'currency' && selectedType !== 'consumable' && (
            <div className="flex items-center justify-between p-3 bg-gray-50 rounded-lg border border-gray-200">
              <Label htmlFor="attunement" className="text-gray-700 font-medium cursor-pointer">
                Richiede Sintonizzazione
              </Label>
              <Switch
                id="attunement"
                checked={formData.requires_attunement ?? false}
                onCheckedChange={(checked) => setFormData({ ...formData, requires_attunement: checked })}
              />
            </div>
          )}

          {/* Categoria */}
          <div className="col-span-2">
            <Label htmlFor="category" className="text-gray-700 font-medium">
              Categoria (opzionale)
            </Label>
            <Input
              id="category"
              value={formData.category || ''}
              onChange={(e) => setFormData({ ...formData, category: e.target.value })}
              className="mt-1"
              placeholder="es. Arma da mischia, Armatura pesante, Strumento musicale..."
            />
          </div>

          {/* Descrizione */}
          <div className="col-span-2">
            <Label htmlFor="description" className="text-gray-700 font-medium">
              Descrizione
            </Label>
            <Textarea
              id="description"
              value={formData.description || ''}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              className="mt-1 min-h-[100px]"
              placeholder="Descrivi l'oggetto, i suoi effetti speciali, il suo aspetto, ecc."
            />
          </div>
        </div>
        <Separator className="h-[2px] bg-amber-400" />
        {/* Proprietà specifiche per tipo */}
        {renderPropertiesByType()}

        {/* Pulsante submit */}
        <div className="flex justify-end pt-4 border-t border-amber-200">
          <AntiqueButton
            type="submit"
            variant="parchment"
            size="md"
            icon={<Save className="w-4 h-4" />}
            loading={isLoading}
            disabled={isLoading}
          >
            {mode === 'edit' ? 'Salva Modifiche' : 'Crea Oggetto'}
          </AntiqueButton>
        </div>
      </form>
    </AncientContainer>
  );
}