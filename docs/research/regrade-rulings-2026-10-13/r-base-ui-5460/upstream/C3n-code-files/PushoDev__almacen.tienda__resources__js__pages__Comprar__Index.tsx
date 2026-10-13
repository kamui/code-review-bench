import HeadingSmall from '@/components/heading-small';
import InputError from '@/components/input-error';
import { entidadDeCliente, SelectorEntidad } from '@/components/transacciones/entidad';
import {
    AlertDialog,
    AlertDialogCancel,
    AlertDialogContent,
    AlertDialogDescription,
    AlertDialogFooter,
    AlertDialogHeader,
    AlertDialogTitle,
    AlertDialogTrigger,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Calendar } from '@/components/ui/calendar';
import { Checkbox } from '@/components/ui/checkbox';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import {
    Combobox,
    ComboboxChip,
    ComboboxChips,
    ComboboxChipsInput,
    ComboboxContent,
    ComboboxEmpty,
    ComboboxInput,
    ComboboxItem,
    ComboboxList,
} from '@/components/ui/combobox';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel, FieldLegend, FieldSeparator, FieldSet } from '@/components/ui/field';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Separator } from '@/components/ui/separator';
import { Toaster } from '@/components/ui/sileo-toaster';
import { Table, TableBody, TableCell, TableFooter, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import AppLayout from '@/layouts/app-layout';
import { sileo } from '@/lib/sileo';
import { cn } from '@/lib/utils';
import { AlmacenProps, CategoriasProps, ClienteProps, CuentaNegocioProps, type BreadcrumbItem } from '@/types';
import { Head, Link, useForm, usePage } from '@inertiajs/react';
import axios from 'axios';
import { format } from 'date-fns';
import {
    AlertTriangle,
    CalendarIcon,
    CheckCircle,
    Clock,
    CreditCard,
    DollarSign,
    Edit2,
    HardDriveUpload,
    Info,
    Loader2,
    Package,
    Phone,
    PlusCircle,
    PlusIcon,
    ShoppingBasket,
    ShoppingCart,
    Trash2Icon,
    Users,
    Wallet,
    Warehouse,
    X,
    XCircle,
} from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { InputGroup, InputGroupAddon, InputGroupInput } from '@/components/ui/input-group';
import { ScrollProgress } from '@/components/ui/scroll';
import { Spinner } from '@/components/ui/spinner';
import { Textarea } from '@/components/ui/textarea';

interface CompraReciente {
    id: number;
    fecha_compra: string;
    total_compra: number;
    tipo_compra: 'pago_cash' | 'deuda_proveedor';
    estado: 'pendiente' | 'aprobada' | 'anulada';
    // Cliente al que se le compró
    proveedor: string | null;
    es_parcial: boolean;
}

const ESTADO_COMPRA_CONFIG = {
    pendiente: { label: 'Pendiente', className: 'bg-amber-500 text-white hover:opacity-90', icon: Clock },
    aprobada: { label: 'Aprobada', className: 'bg-emerald-600 text-white hover:opacity-90', icon: CheckCircle },
    anulada: { label: 'Anulada', className: 'bg-red-500 text-white hover:opacity-90', icon: XCircle },
} as const;

interface ProductoExistente {
    id: number;
    nombre_producto: string;
    marca_producto: string | null;
    modelo_producto: string | null;
    capacidad_producto: string | null;
    categoria: string | null;
    precio_compra_producto: number;
    cantidad_total: number;
    stock_por_almacen: { almacen_id: number; nombre_almacen: string; cantidad: number }[];
}

// Dropdown de sugerencias de productos existentes, compartido por los 3 campos que
// disparan autocompletado (Nombre, Marca, Modelo). Componente top-level (no anidado
// dentro de ComprarPage) para no perder estado en cada re-render del formulario.
function ProductoSugerenciasDropdown({
    mostrar,
    buscando,
    sugerencias,
    resaltada,
    onResaltar,
    onSeleccionar,
}: {
    mostrar: boolean;
    buscando: boolean;
    sugerencias: ProductoExistente[];
    resaltada: number;
    onResaltar: (index: number) => void;
    onSeleccionar: (p: ProductoExistente) => void;
}) {
    if (!mostrar) return null;

    return (
        <div className="bg-popover absolute top-full left-0 z-50 mt-1 w-full max-w-sm rounded-md border shadow-lg">
            {buscando ? (
                <div className="text-muted-foreground flex items-center justify-center gap-2 p-3 text-sm">
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Buscando...
                </div>
            ) : sugerencias.length > 0 ? (
                <ul className="max-h-64 overflow-y-auto py-1">
                    {sugerencias.map((p, index) => (
                        <li
                            key={p.id}
                            onMouseDown={(e) => {
                                e.preventDefault();
                                onSeleccionar(p);
                            }}
                            onMouseEnter={() => onResaltar(index)}
                            className={cn('cursor-pointer px-3 py-2 text-sm', index === resaltada ? 'bg-accent' : 'hover:bg-accent')}
                        >
                            <div className="flex items-center justify-between gap-2">
                                <span className="font-medium">{p.nombre_producto}</span>
                                <Badge variant={p.cantidad_total > 0 ? 'outline' : 'destructive'} className="shrink-0 text-xs">
                                    {p.cantidad_total} en stock
                                </Badge>
                            </div>
                            <p className="text-muted-foreground text-xs">
                                {[p.marca_producto, p.modelo_producto, p.capacidad_producto].filter(Boolean).join(' · ') || 'Sin especificaciones'}
                                {' — Costo actual: '}
                                <span className="font-medium">${p.precio_compra_producto.toFixed(2)}</span>
                            </p>
                        </li>
                    ))}
                </ul>
            ) : (
                <p className="text-muted-foreground p-3 text-sm">No existe — se creará un producto nuevo.</p>
            )}
        </div>
    );
}

// =================================================================
// 🚨 ATRIBUTOS DE PRODUCTO ACTUALIZADOS EN TYPESCRIPT
// =================================================================
export interface ProductoComprarProps {
    id: number;
    almacen_id: number;
    producto: string;
    marca?: string;
    modelo?: string;
    capacidad?: string;
    color?: string;
    categoria: string;
    codigo: string;
    codigo_barras?: string;
    cantidad: number;
    precio: number;
}
// =================================================================

// =================================================================
// ⚡ FUNCIÓN DE GENERACIÓN DE CÓDIGO EN FRONTEND
// =================================================================
const generarCodigoLocal = (producto: string, marca: string, modelo: string, capacidad: string): string => {
    const cleanAndTruncate = (value: string | undefined): string => {
        if (!value) return '';
        const cleaned = value.toUpperCase().replace(/[^A-Z0-9]/g, '');
        return cleaned.substring(0, 3);
    };

    const nombre = cleanAndTruncate(producto).replace(/[^A-Z]/g, '');
    const marcaClean = cleanAndTruncate(marca);
    const modeloClean = cleanAndTruncate(modelo);
    const capacidadNumeros = (capacidad || '').replace(/[^0-9]/g, '');

    const nombrePadded = nombre.padEnd(3, 'X');
    const marcaPadded = marcaClean.padEnd(3, 'X');
    const modeloPadded = modeloClean.padEnd(3, 'X');

    let parteFija = nombrePadded + marcaPadded + modeloPadded + capacidadNumeros;

    if (parteFija.length > 14) {
        parteFija = parteFija.substring(0, 14);
    }

    let codigo = parteFija;
    const longitudRestante = 14 - codigo.length;

    if (longitudRestante > 0) {
        let randomDigits = '';
        for (let i = 0; i < longitudRestante; i++) {
            randomDigits += Math.floor(Math.random() * 10).toString();
        }
        codigo += randomDigits;
    }

    return codigo.substring(0, 14);
};
// =================================================================

const breadcrumbs: BreadcrumbItem[] = [
    {
        title: 'Caja Principal',
        href: '/dashboard',
    },
    {
        title: 'Productos',
        href: '/listado-productos',
    },
    {
        title: 'Realizar Venta',
        href: 'punto-venta',
    },
    {
        title: 'Adquirir Nuevos Productos',
        href: '/comprar',
    },
];

export default function ComprarPage() {
    const { props } = usePage();
    const { errors } = props;
    const comprasRecientes: CompraReciente[] = (props as any).compras_recientes ?? [];
    const [almacens, setAlmacens] = useState<AlmacenProps[]>([]);
    const [categorias, setCategorias] = useState<CategoriasProps[]>([]);
    const [cuentas, setCuentas] = useState<CuentaNegocioProps[]>([]);
    const [date, setDate] = useState<Date | undefined>(new Date());
    const [editingProductId, setEditingProductId] = useState<number | null>(null);
    const [clientes, setClientes] = useState<ClienteProps[]>([]);
    const [loading, setLoading] = useState(true);

    // Estados para búsqueda de clientes
    const [clienteSearchTerm, setClienteSearchTerm] = useState('');
    const [filteredClientes, setFilteredClientes] = useState<ClienteProps[]>([]);
    const [isSearchingClientes, setIsSearchingClientes] = useState(false);

    // Estados para autocompletado de productos existentes (evita duplicados y avisa antes de pisar el costo).
    // Un solo estado compartido por los 3 campos que pueden disparar la búsqueda (Nombre/Marca/Modelo) —
    // `campoEnFoco` indica cuál de los tres está activo y por lo tanto cuál dropdown se muestra.
    const [productoSugerencias, setProductoSugerencias] = useState<ProductoExistente[]>([]);
    const [buscandoProducto, setBuscandoProducto] = useState(false);
    const [campoEnFoco, setCampoEnFoco] = useState<'producto' | 'marca' | 'modelo' | null>(null);
    // Índice resaltado del dropdown de sugerencias (teclado ↑/↓ y hover del mouse comparten el mismo estado
    // para que ambos métodos de selección se sientan consistentes). -1 = nada resaltado.
    const [sugerenciaResaltada, setSugerenciaResaltada] = useState(-1);
    const [productoCoincidente, setProductoCoincidente] = useState<ProductoExistente | null>(null);
    const justSelectedProductoRef = useRef(false);

    // Fix del Combobox (@base-ui/react) roto dentro de un AlertDialog (Radix): el popup se porta a
    // <body> por defecto, queda fuera del focus-trap del diálogo y el click con mouse no selecciona
    // nada (ver docs/pendiente-combobox-reemplazo.md, mismo caso ya resuelto en ventas/PaymentForm.tsx).
    // El diálogo "Editar Producto" vive dentro de un .map() por fila del carrito, así que no se puede
    // usar useRef ahí adentro — se resuelve con un ref-callback a nivel de componente en su lugar.
    const [editDialogContainer, setEditDialogContainer] = useState<HTMLElement | undefined>(undefined);
    const resolveEditDialogContainer = (node: HTMLElement | null) => {
        const container = node?.closest('[data-slot="alert-dialog-content"]');
        if (container instanceof HTMLElement) {
            setEditDialogContainer(container);
        }
    };

    // Mismo fix que arriba, para el diálogo "Realizar Compra" (paso "Procesar Pago"). Este no vive
    // dentro de un .map(), pero se resuelve igual con un ref-callback para no mezclar dos patrones
    // distintos de un mismo problema en el mismo archivo.
    const [pagoDialogContainer, setPagoDialogContainer] = useState<HTMLElement | undefined>(undefined);
    const resolvePagoDialogContainer = (node: HTMLElement | null) => {
        const container = node?.closest('[data-slot="alert-dialog-content"]');
        if (container instanceof HTMLElement) {
            setPagoDialogContainer(container);
        }
    };

    // Estados para búsqueda de almacenes
    const [lastSelectedAlmacenId, setLastSelectedAlmacenId] = useState<string>('');

    // Estado para el modal de crear almacén
    const [isCrearAlmacenDialogOpen, setIsCrearAlmacenDialogOpen] = useState(false);

    // Estado para el modal de crear categoría
    const [isCrearCategoriaDialogOpen, setIsCrearCategoriaDialogOpen] = useState(false);

    const [tempFormData, setTempFormData] = useState<
        Omit<ProductoComprarProps, 'id' | 'almacen_id' | 'precio' | 'cantidad'> & { almacen_id: string; precio: string; cantidad: string }
    >({
        almacen_id: '',
        producto: '',
        marca: '',
        modelo: '',
        capacidad: '',
        color: '',
        categoria: '',
        codigo: '',
        cantidad: '',
        precio: '',
    });

    const [productos, setProductos] = useState<ProductoComprarProps[]>([]);
    const [isDialogOpen, setIsDialogOpen] = useState(false);

    const { data, setData, post, processing } = useForm({
        compra: '',
        proveedor: '',
        fecha: date ? date.toISOString().split('T')[0] : '',
        pagos: [] as { cuenta_id: number; monto: number }[],
        pagos_clientes: [] as { cliente_id: number; monto: number }[],
        productos: [] as ProductoComprarProps[],
        permitir_deuda_parcial: false,
    });

    // Estado para el modal de crear cliente
    const [isCrearClienteDialogOpen, setIsCrearClienteDialogOpen] = useState(false);
    // true cuando el modal se abrió desde el selector "Cliente" de la compra (y no desde el financiamiento)
    const [crearComoClienteDeCompra, setCrearComoClienteDeCompra] = useState(false);

    // 🔍 EFECTO PARA BÚSQUEDA EN TIEMPO REAL DE CLIENTES
    useEffect(() => {
        const searchClientes = async () => {
            if (clienteSearchTerm.length >= 2) {
                setIsSearchingClientes(true);
                try {
                    const response = await axios.get(route('compras.clientes.fisicos'), {
                        params: { search: clienteSearchTerm },
                    });
                    setFilteredClientes(response.data);
                } catch (error) {
                    console.error('Error buscando clientes:', error);
                    sileo.error({ title: 'Error al buscar clientes' });
                } finally {
                    setIsSearchingClientes(false);
                }
            } else {
                setFilteredClientes(clientes);
            }
        };

        const debounceTimer = setTimeout(searchClientes, 300);
        return () => clearTimeout(debounceTimer);
    }, [clienteSearchTerm, clientes]);

    // 🔍 EFECTO PARA AUTOCOMPLETADO DE PRODUCTOS EXISTENTES — dispara desde Nombre, Marca o Modelo,
    // el que tenga el foco en ese momento (el backend ya busca por los 3 campos, ver Producto::scopeBuscar).
    useEffect(() => {
        // Evita re-buscar/reabrir el dropdown cuando el cambio de texto vino de seleccionar una sugerencia
        if (justSelectedProductoRef.current) {
            justSelectedProductoRef.current = false;
            return;
        }

        if (!campoEnFoco) return;

        const termino = tempFormData[campoEnFoco].trim();
        if (termino.length < 2) {
            setProductoSugerencias([]);
            setSugerenciaResaltada(-1);
            return;
        }

        const buscarProductos = async () => {
            setBuscandoProducto(true);
            try {
                const response = await axios.get(route('compras.productos.buscar'), {
                    params: { search: termino },
                });
                setProductoSugerencias(response.data);
                setSugerenciaResaltada(-1);
            } catch (error) {
                console.error('Error buscando productos existentes:', error);
            } finally {
                setBuscandoProducto(false);
            }
        };

        const debounceTimer = setTimeout(buscarProductos, 300);
        return () => clearTimeout(debounceTimer);
    }, [tempFormData.producto, tempFormData.marca, tempFormData.modelo, campoEnFoco]);

    // 🔍 EFECTO PARA BÚSQUEDA EN TIEMPO REAL DE ALMACENES - ELIMINADO: El Combobox maneja el filtrado nativamente
    // 🔍 EFECTO PARA BÚSQUEDA EN TIEMPO REAL DE CUENTAS - ELIMINADO: mismo motivo, ver Combobox de "Pago desde Cuentas"

    // ⚡ EFECTO PARA MANTENER EL ÚLTIMO ALMACÉN SELECCIONADO
    useEffect(() => {
        if (lastSelectedAlmacenId && !tempFormData.almacen_id) {
            setTempFormData((prev) => ({
                ...prev,
                almacen_id: lastSelectedAlmacenId,
            }));
        }
    }, [lastSelectedAlmacenId, tempFormData.almacen_id]);

    useEffect(() => {
        const productosParaBackend = productos.map((p) => ({
            ...p,
            almacen_id: parseInt(p.almacen_id as any),
            codigo_barras: (p.codigo || '').trim(),
        }));
        setData('productos', productosParaBackend as ProductoComprarProps[]);
    }, [productos]);

    useEffect(() => {
        const cargarDatos = async () => {
            try {
                setLoading(true);

                const [almacenesRes, categoriasRes, cuentasRes, clientesRes] = await Promise.all([
                    axios.get(route('compras.almacenes')),
                    axios.get(route('compras.categorias')),
                    axios.get(route('compras.cuentas.pago')),
                    axios.get(route('compras.clientes.fisicos')),
                ]);

                setAlmacens(almacenesRes.data);
                setCategorias(categoriasRes.data);
                setCuentas(cuentasRes.data);
                setClientes(clientesRes.data);
                setFilteredClientes(clientesRes.data);
            } catch (error) {
                console.error('Error al cargar datos:', error);
                sileo.error({ title: 'Error al cargar los datos necesarios' });
            } finally {
                setLoading(false);
            }
        };

        cargarDatos();
    }, []);

    const handleTempInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        const { name, value } = e.target;
        setTempFormData((prev) => ({
            ...prev,
            [name]: name === 'precio' || name === 'cantidad' ? value : value.toUpperCase(),
        }));
        // Si el usuario sigue editando a mano después de haber elegido una sugerencia, el aviso queda desactualizado
        if (productoCoincidente && (name === 'producto' || name === 'marca' || name === 'modelo')) {
            setProductoCoincidente(null);
        }
    };

    const handleTempSelectChange = (name: string, value: string) => {
        setTempFormData((prev) => ({
            ...prev,
            [name]: value,
        }));

        // Guardar el último almacén seleccionado
        if (name === 'almacen_id') {
            setLastSelectedAlmacenId(value);
        }
    };

    const resetTempForm = () => {
        setTempFormData({
            almacen_id: lastSelectedAlmacenId || '', // Mantener el último almacén seleccionado
            producto: '',
            marca: '',
            modelo: '',
            capacidad: '',
            color: '',
            categoria: '',
            codigo: '',
            cantidad: '',
            precio: '',
        });
        setProductoCoincidente(null);
        setCampoEnFoco(null);
    };

    // Autocompleta marca/modelo/capacidad/categoría desde un producto ya existente, sin importar si la
    // sugerencia vino del campo Nombre, Marca o Modelo. El color y el precio NO se rellenan: el color es
    // lo que suele distinguir la unidad real, y el precio se deja en blanco a propósito para que el
    // usuario compare contra el costo actual (mostrado en el panel de abajo) en vez de arrastrarlo sin darse cuenta.
    const seleccionarProductoExistente = (producto: ProductoExistente) => {
        justSelectedProductoRef.current = true;
        setTempFormData((prev) => ({
            ...prev,
            producto: producto.nombre_producto,
            marca: producto.marca_producto || '',
            modelo: producto.modelo_producto || '',
            capacidad: producto.capacidad_producto || '',
            categoria: producto.categoria || '',
        }));
        setProductoCoincidente(producto);
        setCampoEnFoco(null);
        setProductoSugerencias([]);
    };

    const agregarProducto = () => {
        const cant = parseInt(tempFormData.cantidad) || 0;
        const prec = parseFloat(tempFormData.precio) || 0;

        if (!tempFormData.producto || !tempFormData.categoria || cant <= 0 || prec <= 0 || !tempFormData.almacen_id) {
            sileo.warning({
                title: 'Faltan datos',
                description: 'Completa los campos obligatorios (Producto, Categoría, Cantidad, Precio y Almacén).',
            });
            return;
        }

        const nuevoCodigo = generarCodigoLocal(
            tempFormData.producto,
            tempFormData.marca || '',
            tempFormData.modelo || '',
            tempFormData.capacidad || '',
        );

        const nuevoProducto: ProductoComprarProps = {
            id: editingProductId || Date.now(),
            almacen_id: parseInt(tempFormData.almacen_id as any),
            producto: tempFormData.producto,
            marca: tempFormData.marca,
            modelo: tempFormData.modelo,
            capacidad: tempFormData.capacidad,
            color: tempFormData.color,
            categoria: tempFormData.categoria,
            codigo: nuevoCodigo,
            cantidad: cant,
            precio: prec,
        };

        if (editingProductId) {
            setProductos((prev) => prev.map((p) => (p.id === editingProductId ? nuevoProducto : p)));
            setEditingProductId(null);
            sileo.success({ title: 'Producto actualizado', description: `${nuevoProducto.producto} se actualizó en la lista.` });
        } else {
            setProductos((prev) => [...prev, nuevoProducto]);
            sileo.success({ title: 'Producto agregado', description: `${nuevoProducto.producto} se agregó a la lista de compra.` });
        }

        resetTempForm();
    };

    // Enter en cualquier campo de texto del formulario de alta agrega el producto, para no
    // depender del mouse al cargar varias líneas seguidas. Los Combobox (Almacén/Categoría)
    // quedan afuera a propósito: Enter ahí ya lo usa el propio combobox para seleccionar.
    const handleEnterAgregarProducto = (e: React.KeyboardEvent<HTMLInputElement>) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            agregarProducto();
        }
    };

    // Igual que handleEnterAgregarProducto, pero para los 3 campos (Nombre/Marca/Modelo) que además
    // disparan el dropdown de sugerencias de productos existentes: cuando el dropdown está abierto,
    // ↑/↓ mueven el resaltado y Enter selecciona la sugerencia resaltada en vez de agregar un producto
    // nuevo (antes Enter siempre creaba una línea nueva, ignorando el dropdown por completo — el usuario
    // que autocompletaba por teclado no tenía forma de elegir una sugerencia existente).
    const handleAutocompleteKeyDown = (e: React.KeyboardEvent<HTMLInputElement>, campo: 'producto' | 'marca' | 'modelo') => {
        const dropdownAbierto = campoEnFoco === campo && tempFormData[campo].trim().length >= 2 && productoSugerencias.length > 0;

        if (dropdownAbierto) {
            if (e.key === 'ArrowDown') {
                e.preventDefault();
                setSugerenciaResaltada((i) => (i + 1) % productoSugerencias.length);
                return;
            }
            if (e.key === 'ArrowUp') {
                e.preventDefault();
                setSugerenciaResaltada((i) => (i <= 0 ? productoSugerencias.length - 1 : i - 1));
                return;
            }
            if (e.key === 'Enter' && sugerenciaResaltada >= 0) {
                e.preventDefault();
                seleccionarProductoExistente(productoSugerencias[sugerenciaResaltada]);
                return;
            }
            if (e.key === 'Escape') {
                setCampoEnFoco(null);
                return;
            }
        }

        handleEnterAgregarProducto(e);
    };

    // Preview del código que se autogenera al agregar — mismo cálculo que agregarProducto(),
    // solo que en vivo, para que no sea invisible hasta abrir "Editar" después de cargado.
    const codigoPreview = tempFormData.producto
        ? generarCodigoLocal(tempFormData.producto, tempFormData.marca || '', tempFormData.modelo || '', tempFormData.capacidad || '')
        : '';

    const eliminarProducto = (id: number) => {
        const producto = productos.find((p) => p.id === id);
        setProductos((prev) => prev.filter((p) => p.id !== id));
        if (producto) {
            sileo.info({ title: 'Producto quitado', description: `${producto.producto} se quitó de la lista.` });
        }
    };

    const editarProducto = (id: number) => {
        const productoParaEditar = productos.find((p) => p.id === id);
        if (productoParaEditar) {
            setTempFormData({
                almacen_id: productoParaEditar.almacen_id.toString(),
                producto: productoParaEditar.producto,
                marca: productoParaEditar.marca || '',
                modelo: productoParaEditar.modelo || '',
                capacidad: productoParaEditar.capacidad || '',
                color: productoParaEditar.color || '',
                categoria: productoParaEditar.categoria,
                codigo: productoParaEditar.codigo,
                cantidad: String(productoParaEditar.cantidad),
                precio: String(productoParaEditar.precio),
            });
            setEditingProductId(id);
            setIsDialogOpen(true);
        }
    };

    const handleActualizarProducto = () => {
        const cant = parseInt(tempFormData.cantidad) || 0;
        const prec = parseFloat(tempFormData.precio) || 0;

        if (!tempFormData.producto.trim() || !tempFormData.categoria.trim() || cant <= 0 || prec <= 0 || !tempFormData.almacen_id) {
            sileo.warning({
                title: 'Faltan datos',
                description: 'Completa los campos obligatorios válidos (Producto, Categoría, Cantidad, Precio y Almacén).',
            });
            return;
        }

        setProductos((prev) =>
            prev.map((prod) =>
                prod.id === editingProductId
                    ? ({
                          id: editingProductId,
                          almacen_id: parseInt(tempFormData.almacen_id),
                          producto: tempFormData.producto,
                          marca: tempFormData.marca,
                          modelo: tempFormData.modelo,
                          capacidad: tempFormData.capacidad,
                          color: tempFormData.color,
                          categoria: tempFormData.categoria,
                          codigo: tempFormData.codigo,
                          cantidad: cant,
                          precio: prec,
                      } as ProductoComprarProps)
                    : prod,
            ),
        );

        resetTempForm();
        setEditingProductId(null);
        sileo.success({ title: 'Producto actualizado', description: 'Los cambios se guardaron correctamente.' });
        setIsDialogOpen(false);
    };

    const calcularTotal = () => {
        return productos.reduce((total, p) => total + p.cantidad * p.precio, 0).toFixed(2);
    };

    const clienteDeCompra = clientes.find((c) => c.nombre_cliente === data.proveedor) || null;
    // Cada cliente se muestra como en Transacciones: insignia + badge con su saldo
    const entidadesCliente = clientes.map((c) =>
        entidadDeCliente({ id: c.id, nombre_cliente: c.nombre_cliente, deuda_pago_cliente: c.deuda_pago_cliente ?? 0 }),
    );
    const selectedAlmacen = almacens.find((a) => a.id.toString() === tempFormData.almacen_id) || null;
    const selectedCategoria = categorias.find((c) => c.nombre_categoria === tempFormData.categoria) || null;

    const realizarCompra = () => {
        if (productos.length === 0) {
            sileo.warning({ title: 'Debe agregar al menos un producto para realizar la compra.' });
            return;
        }

        if (!data.proveedor || !date) {
            sileo.warning({ title: 'Por favor, complete la Fecha y el Asociado.' });
            return;
        }

        setData('fecha', format(date, 'yyyy-MM-dd'));

        // Swap directo, sin sileo.promise(): esta llamada mueve dinero/stock real en el backend
        // (CompraController::store) y no se puede verificar en navegador en esta sesión — mismo
        // criterio de cautela ya aplicado en Vendor/Index.tsx para su checkout.
        post(route('comprar.store'), {
            preserveScroll: true,
            onSuccess: () => {
                setProductos([]);
                resetTempForm();
                sileo.success({
                    title: 'Compra realizada exitosamente',
                    description: 'Los productos han sido agregados al inventario.',
                });
            },
            onError: (errors) => {
                // Los errores de validación del backend vienen por índice real del carrito
                // (productos.2.precio, productos.5.cantidad, etc.) — antes solo se leía el
                // índice 0, así que si fallaba cualquier otra línea, el usuario no se enteraba
                // ni de qué producto era ni de qué campo. Acá se busca cualquier error
                // "productos.N.campo" y se señala el producto real por nombre.
                const claveErrorProducto = Object.keys(errors).find((clave) => /^productos\.\d+\./.test(clave));

                if (claveErrorProducto) {
                    const indice = parseInt(claveErrorProducto.split('.')[1], 10);
                    const nombreProducto = productos[indice]?.producto ?? `línea ${indice + 1}`;
                    sileo.error({
                        title: `Error en "${nombreProducto}"`,
                        description: errors[claveErrorProducto],
                    });
                } else if (errors.error) {
                    sileo.error({
                        title: 'Error al procesar la compra',
                        description: errors.error,
                    });
                } else {
                    sileo.error({ title: 'Ocurrió un error inesperado al procesar la compra.' });
                }
            },
        });
    };

    // 🏗️ COMPONENTE DE CREACIÓN DE ALMACÉN - CORREGIDO CON CAMPOS REALES
    const CrearAlmacenDialogContent = () => {
        const [localAlmacen, setLocalAlmacen] = useState({
            nombre_almacen: '',
            tipo_almacen: 'punto_venta',
            telefono_almacen: '',
            correo_almacen: '',
            provincia_almacen: '',
            ciudad_almacen: '',
            notas_almacen: '',
        });

        const [localErrors, setLocalErrors] = useState<Record<string, string>>({});

        const handleLocalChange = (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
            const { name, value } = e.target;
            setLocalAlmacen((prev) => ({
                ...prev,
                [name]:
                    name === 'nombre_almacen' || name === 'provincia_almacen' || name === 'ciudad_almacen' || name === 'notas_almacen'
                        ? value.toUpperCase()
                        : name === 'correo_almacen'
                          ? value.toLowerCase()
                          : value,
            }));
        };

        const handleLocalSelectChange = (name: string, value: string) => {
            setLocalAlmacen((prev) => ({
                ...prev,
                [name]: value,
            }));
        };

        const crearAlmacenLocal = async () => {
            // Validación básica en frontend
            if (!localAlmacen.nombre_almacen.trim() || !localAlmacen.telefono_almacen.trim()) {
                sileo.error({ title: 'Nombre y teléfono son requeridos' });
                return;
            }

            try {
                const response = await axios.post(route('compras.almacen.store'), {
                    nombre_almacen: localAlmacen.nombre_almacen.trim().toUpperCase(),
                    tipo_almacen: localAlmacen.tipo_almacen,
                    telefono_almacen: localAlmacen.telefono_almacen.trim(),
                    correo_almacen: localAlmacen.correo_almacen.trim().toLowerCase(),
                    provincia_almacen: localAlmacen.provincia_almacen.trim().toUpperCase(),
                    ciudad_almacen: localAlmacen.ciudad_almacen.trim().toUpperCase(),
                    notas_almacen: localAlmacen.notas_almacen.trim().toUpperCase(),
                });

                const { almacen, message, existe } = response.data;

                if (existe) {
                    sileo.warning({ title: message, description: 'Se usará el almacén existente en el sistema.' });
                } else {
                    sileo.success({ title: message, description: 'Almacén creado exitosamente.' });
                }

                // Agregar a la lista de almacenes si no está (ya existente o recién creado)
                setAlmacens((prev) => (prev.some((a) => a.id === almacen.id) ? prev : [...prev, almacen]));

                // Seleccionar automáticamente el almacén (nuevo o existente)
                setLastSelectedAlmacenId(almacen.id.toString());
                setTempFormData((prev) => ({
                    ...prev,
                    almacen_id: almacen.id.toString(),
                }));

                // Limpiar formulario y cerrar diálogo
                setLocalAlmacen({
                    nombre_almacen: '',
                    tipo_almacen: 'punto_venta',
                    telefono_almacen: '',
                    correo_almacen: '',
                    provincia_almacen: '',
                    ciudad_almacen: '',
                    notas_almacen: '',
                });

                setLocalErrors({});
                setIsCrearAlmacenDialogOpen(false);
            } catch (error: any) {
                console.error('Error al crear almacén:', error);

                if (error.response?.data?.errors) {
                    setLocalErrors(error.response.data.errors);
                    sileo.error({ title: 'Error de validación', description: 'Por favor corrige los errores en el formulario.' });
                } else {
                    sileo.error({ title: 'Error al crear almacén', description: 'Intenta nuevamente o contacta al administrador.' });
                }
            }
        };

        const resetDialog = () => {
            setLocalAlmacen({
                nombre_almacen: '',
                tipo_almacen: 'punto_venta',
                telefono_almacen: '',
                correo_almacen: '',
                provincia_almacen: '',
                ciudad_almacen: '',
                notas_almacen: '',
            });
            setLocalErrors({});
            setIsCrearAlmacenDialogOpen(false);
        };

        return (
            <DialogContent className="max-h-[190vh] sm:max-w-lg">
                {/* HEADER ELEGANTE */}
                <DialogHeader className="border-b bg-gradient-to-r from-blue-600 to-blue-700 px-6 py-5 text-white">
                    <div className="flex items-center gap-3">
                        <div className="flex h-12 w-12 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                            <Warehouse className="h-7 w-7" />
                        </div>
                        <div>
                            <DialogTitle className="text-2xl font-bold">Crear Nuevo Almacén</DialogTitle>
                            <DialogDescription className="text-blue-100">
                                Configura un nuevo espacio de almacenamiento para tus productos
                            </DialogDescription>
                        </div>
                    </div>
                </DialogHeader>

                {/* CONTENIDO CON SCROLL PERFECTO */}
                <ScrollArea className="max-h-[60vh] px-6 py-6">
                    <div className="space-y-8">
                        {/* INFORMACIÓN PRINCIPAL */}
                        <FieldSet>
                            <FieldLegend className="text-lg font-semibold">Información Principal</FieldLegend>

                            <FieldGroup className="space-y-6">
                                {/* Nombre del Almacén */}
                                <Field>
                                    <FieldLabel className="flex items-center gap-1">
                                        Nombre del Almacén <span className="text-red-500">*</span>
                                    </FieldLabel>
                                    <Input
                                        id="dialog-nombre-almacen"
                                        name="nombre_almacen"
                                        value={localAlmacen.nombre_almacen}
                                        onChange={handleLocalChange}
                                        placeholder="Ej: ALMACÉN CENTRAL, BODEGA NORTE"
                                        className={localErrors.nombre_almacen ? 'border-red-500 focus-visible:ring-red-500' : ''}
                                    />
                                    {localErrors.nombre_almacen && <FieldError>{localErrors.nombre_almacen}</FieldError>}
                                </Field>

                                {/* Tipo de Almacén */}
                                <Field>
                                    <FieldLabel className="flex items-center gap-1">
                                        Tipo de Almacén <span className="text-red-500">*</span>
                                    </FieldLabel>
                                    <Select
                                        value={localAlmacen.tipo_almacen}
                                        onValueChange={(value) => handleLocalSelectChange('tipo_almacen', value)}
                                    >
                                        <SelectTrigger className="h-11">
                                            <SelectValue placeholder="Seleccione tipo" />
                                        </SelectTrigger>
                                        <SelectContent>
                                            <SelectItem value="almacen">Almacén</SelectItem>
                                            <SelectItem value="punto_venta">Punto de Venta</SelectItem>
                                            <SelectItem value="transportacion">Transportación</SelectItem>
                                        </SelectContent>
                                    </Select>
                                    {localErrors.tipo_almacen && <FieldError>{localErrors.tipo_almacen}</FieldError>}
                                </Field>

                                {/* Teléfono */}
                                <Field>
                                    <FieldLabel className="flex items-center gap-1">
                                        Teléfono <span className="text-red-500">*</span>
                                    </FieldLabel>
                                    <InputGroup>
                                        <InputGroupAddon>
                                            <Phone className="text-muted-foreground h-4 w-4" />
                                        </InputGroupAddon>
                                        <InputGroupInput
                                            id="dialog-telefono-almacen"
                                            name="telefono_almacen"
                                            value={localAlmacen.telefono_almacen}
                                            onChange={handleLocalChange}
                                            placeholder="Ej: 555-1234"
                                            className={localErrors.telefono_almacen ? 'border-red-500' : ''}
                                        />
                                    </InputGroup>
                                    {localErrors.telefono_almacen && <FieldError>{localErrors.telefono_almacen}</FieldError>}
                                </Field>

                                {/* Correo */}
                                <Field>
                                    <FieldLabel>Correo Electrónico</FieldLabel>
                                    <InputGroup>
                                        <InputGroupAddon>@</InputGroupAddon>
                                        <InputGroupInput
                                            id="dialog-correo-almacen"
                                            name="correo_almacen"
                                            type="email"
                                            value={localAlmacen.correo_almacen}
                                            onChange={handleLocalChange}
                                            placeholder="ejemplo@empresa.com"
                                        />
                                    </InputGroup>
                                </Field>
                            </FieldGroup>
                        </FieldSet>

                        <FieldSeparator />
                        {/* UBICACIÓN */}
                        <FieldSet>
                            <FieldLegend>Ubicación</FieldLegend>
                            <FieldGroup className="grid grid-cols-2 gap-4">
                                <Field>
                                    <FieldLabel>Provincia</FieldLabel>
                                    <Input
                                        id="dialog-provincia-almacen"
                                        name="provincia_almacen"
                                        value={localAlmacen.provincia_almacen}
                                        onChange={handleLocalChange}
                                        placeholder="Ej: GRANMA, MAYABEQUE"
                                    />
                                </Field>
                                <Field>
                                    <FieldLabel>Ciudad</FieldLabel>
                                    <Input
                                        id="dialog-ciudad-almacen"
                                        name="ciudad_almacen"
                                        value={localAlmacen.ciudad_almacen}
                                        onChange={handleLocalChange}
                                        placeholder="Ej: MANZANILLO, QUIVICÁN"
                                    />
                                </Field>
                            </FieldGroup>
                        </FieldSet>

                        <FieldSeparator />
                        {/* NOTAS */}
                        <FieldSet>
                            <FieldLegend>Notas Adicionales</FieldLegend>
                            <Field>
                                <Textarea
                                    id="dialog-notas-almacen"
                                    name="notas_almacen"
                                    value={localAlmacen.notas_almacen}
                                    onChange={handleLocalChange}
                                    placeholder="INFORMACIÓN ADICIONAL SOBRE EL ALMACÉN..."
                                    rows={3}
                                    className="resize-none"
                                />
                            </Field>
                        </FieldSet>

                        {/* CAJA DE INFO BONITA */}
                        <div className="rounded-xl border border-blue-200 bg-gradient-to-br from-blue-50 to-blue-100 p-5 dark:border-blue-800 dark:from-blue-950/50 dark:to-blue-900/50">
                            <div className="flex items-start gap-3">
                                <Info className="mt-0.5 h-6 w-6 text-blue-600 dark:text-blue-400" />
                                <div className="space-y-2 text-sm">
                                    <p className="font-semibold text-blue-900 dark:text-blue-100">Tipos de almacén:</p>
                                    <ul className="list-disc space-y-1 pl-5 text-blue-800 dark:text-blue-200">
                                        <li>
                                            <strong>Almacén:</strong> Para guardar productos en inventario
                                        </li>
                                        <li>
                                            <strong>Punto de Venta:</strong> Para venta directa al público
                                        </li>
                                        <li>
                                            <strong>Transportación:</strong> Para movilización de productos
                                        </li>
                                    </ul>
                                </div>
                            </div>
                        </div>
                    </div>
                </ScrollArea>

                {/* FOOTER FIJO Y ELEGANTE */}
                <DialogFooter className="bg-background border-t px-6 py-5">
                    <Button variant="outline" onClick={resetDialog} className="h-11 px-6">
                        Cancelar
                    </Button>
                    <Button
                        onClick={crearAlmacenLocal}
                        disabled={!localAlmacen.nombre_almacen.trim() || !localAlmacen.telefono_almacen.trim()}
                        className="h-11 bg-gradient-to-r from-blue-600 to-blue-700 px-8 hover:from-blue-700 hover:to-blue-800"
                    >
                        <CheckCircle className="mr-2 h-5 w-5" />
                        Crear Almacén
                    </Button>
                </DialogFooter>
            </DialogContent>
        );
    };

    // 🆕 COMPONENTE DE CREACIÓN DE CATEGORÍA
    const CrearCategoriaDialogContent = () => {
        const [nombreCategoria, setNombreCategoria] = useState('');
        const [localErrors, setLocalErrors] = useState<Record<string, string>>({});

        const crearCategoriaLocal = async () => {
            const nombreMayusculas = nombreCategoria.trim().toUpperCase();

            if (!nombreMayusculas) {
                sileo.error({ title: 'El nombre de la categoría es requerido' });
                return;
            }

            try {
                const response = await axios.post(route('compras.categoria.store'), {
                    nombre_categoria: nombreMayusculas,
                });

                const { categoria, message } = response.data;

                sileo.success({ title: message, description: 'Categoría creada exitosamente.' });

                // Actualizar el estado global de categorías
                setCategorias((prev) => [...prev, categoria]);
                // Seleccionar automáticamente la nueva categoría en el formulario de producto
                handleTempSelectChange('categoria', categoria.nombre_categoria);

                resetDialog();
            } catch (error: any) {
                console.error('Error al crear categoría:', error);
                if (error.response?.data?.errors) {
                    setLocalErrors(error.response.data.errors);
                    sileo.error({ title: 'Error de validación', description: 'Por favor corrige los errores en el formulario.' });
                } else {
                    sileo.error({ title: 'Error al crear categoría', description: error.response?.data?.message || 'Intenta nuevamente.' });
                }
            }
        };

        const resetDialog = () => {
            setNombreCategoria('');
            setLocalErrors({});
            setIsCrearCategoriaDialogOpen(false);
        };

        return (
            <DialogContent className="overflow-hidden p-0 sm:max-w-md">
                <DialogHeader className="border-b bg-gradient-to-r from-purple-600 to-purple-700 px-6 py-5 text-white">
                    <div className="flex items-center gap-3">
                        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                            <HardDriveUpload className="h-5 w-5" />
                        </div>
                        <div>
                            <DialogTitle className="text-xl text-white">Crear Nueva Categoría</DialogTitle>
                            <DialogDescription className="text-purple-100">Añade una nueva categoría para organizar tus productos.</DialogDescription>
                        </div>
                    </div>
                </DialogHeader>
                <div className="grid gap-4 px-6 py-4">
                    <div className="space-y-2">
                        <Label htmlFor="dialog-nombre-categoria">
                            Nombre de la Categoría <span className="text-red-500">*</span>
                        </Label>
                        <Input
                            id="dialog-nombre-categoria"
                            value={nombreCategoria}
                            onChange={(e) => setNombreCategoria(e.target.value.toUpperCase())}
                            placeholder="Ej: SMARTPHONES, LAPTOPS"
                            className={localErrors.nombre_categoria ? 'border-red-500' : ''}
                            autoFocus
                        />
                        {localErrors.nombre_categoria && <p className="text-sm text-red-500">{localErrors.nombre_categoria}</p>}
                    </div>
                </div>
                <DialogFooter className="gap-2 border-t px-6 py-4">
                    <Button type="button" variant="outline" onClick={resetDialog}>
                        Cancelar
                    </Button>
                    <Button type="button" onClick={crearCategoriaLocal} className="bg-purple-600 hover:bg-purple-700">
                        <PlusCircle className="mr-2 h-4 w-4" />
                        Crear Categoría
                    </Button>
                </DialogFooter>
            </DialogContent>
        );
    };

    // Un cliente creado (o encontrado como ya existente) queda visible de inmediato en los selectores
    // de esta página (el de "Cliente" de la compra y el de "Financiamiento con Clientes"), sin recargar.
    const sincronizarClienteEnListas = (cliente: ClienteProps) => {
        setClientes((prev) => (prev.some((c) => c.id === cliente.id) ? prev : [...prev, cliente]));
    };

    // 🆕 COMPONENTE DE CREACIÓN DE CLIENTE
    const CrearClienteDialogContent = () => {
        const [localCliente, setLocalCliente] = useState({
            nombre_cliente: '',
            telefono_cliente: '+123456789',
            direccion_cliente: 'Residencia del Cliente',
            ciudad_cliente: 'Ciudad de Residencia',
        });

        const [localErrors, setLocalErrors] = useState<Record<string, string>>({});

        const handleLocalChange = (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
            const { name, value } = e.target;
            setLocalCliente((prev) => ({
                ...prev,
                [name]: name === 'nombre_cliente' || name === 'ciudad_cliente' || name === 'direccion_cliente' ? value.toUpperCase() : value,
            }));
        };

        const crearClienteLocal = async () => {
            // Validación básica en frontend
            if (!localCliente.nombre_cliente.trim() || !localCliente.telefono_cliente.trim()) {
                sileo.error({ title: 'Nombre y teléfono son requeridos' });
                return;
            }

            try {
                const response = await axios.post(route('compras.cliente.store'), {
                    nombre_cliente: localCliente.nombre_cliente.trim().toUpperCase(),
                    telefono_cliente: localCliente.telefono_cliente.trim(),
                    direccion_cliente: localCliente.direccion_cliente.trim().toUpperCase(),
                    ciudad_cliente: localCliente.ciudad_cliente.trim().toUpperCase(),
                    tipo_cliente: 'fisico',
                });

                const { cliente, existe, message } = response.data;

                // Si el cliente ya existe (retornado por el backend)
                if (existe) {
                    sileo.info({ title: message, description: 'El cliente ya existía en el sistema. Se ha agregado automáticamente.' });

                    sincronizarClienteEnListas(cliente);
                } else {
                    // Cliente nuevo creado
                    sileo.success({ title: message, description: 'Cliente creado exitosamente.' });
                    sincronizarClienteEnListas(cliente);
                }

                if (crearComoClienteDeCompra) {
                    // Creado desde el selector "Cliente" de la compra: queda seleccionado
                    setData('proveedor', cliente.nombre_cliente);
                } else if (!data.pagos_clientes.some((p) => p.cliente_id === cliente.id)) {
                    // Creado desde "Financiamiento con Clientes": se agrega a pagos_clientes con monto 0
                    setData('pagos_clientes', [...data.pagos_clientes, { cliente_id: cliente.id, monto: 0 }]);
                }

                // Limpiar formulario y cerrar diálogo
                setLocalCliente({
                    nombre_cliente: '',
                    telefono_cliente: '',
                    direccion_cliente: '',
                    ciudad_cliente: '',
                });

                setLocalErrors({});
                setIsCrearClienteDialogOpen(false);
            } catch (error: any) {
                console.error('Error al crear cliente:', error);

                if (error.response?.data?.errors) {
                    setLocalErrors(error.response.data.errors);
                    sileo.error({ title: 'Error de validación', description: 'Por favor corrige los errores en el formulario.' });
                } else {
                    sileo.error({ title: 'Error al crear cliente', description: 'Intenta nuevamente o contacta al administrador.' });
                }
            }
        };

        const resetDialog = () => {
            setLocalCliente({
                nombre_cliente: '',
                telefono_cliente: '',
                direccion_cliente: '',
                ciudad_cliente: '',
            });
            setLocalErrors({});
            setIsCrearClienteDialogOpen(false);
        };

        return (
            <DialogContent className="max-h-[190vh] overflow-hidden p-0 sm:max-w-lg">
                <DialogHeader className="border-b bg-gradient-to-r from-sky-600 to-sky-700 px-6 py-5 text-white">
                    <div className="flex items-center gap-3">
                        <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                            <Users className="h-5 w-5" />
                        </div>
                        <div>
                            <DialogTitle className="text-xl text-white">Crear Nuevo Cliente Físico</DialogTitle>
                            <DialogDescription className="text-sky-100">
                                Los clientes físicos pueden usarse como fuente de financiamiento para compras.
                            </DialogDescription>
                        </div>
                    </div>
                </DialogHeader>
                <ScrollArea className="max-h-[70vh] px-6">
                    <div className="grid gap-6 py-4">
                        <div className="space-y-4">
                            <div className="grid gap-4">
                                <div className="space-y-2">
                                    <Label htmlFor="dialog-nombre-cliente" className="flex items-center gap-1">
                                        Nombre Completo <span className="text-red-500">*</span>
                                    </Label>
                                    <Input
                                        id="dialog-nombre-cliente"
                                        name="nombre_cliente"
                                        value={localCliente.nombre_cliente}
                                        onChange={handleLocalChange}
                                        placeholder="Ej: JUAN PÉREZ"
                                        className={localErrors.nombre_cliente ? 'border-red-500' : ''}
                                    />
                                    {localErrors.nombre_cliente && <p className="text-sm text-red-500">{localErrors.nombre_cliente}</p>}
                                </div>

                                <div className="space-y-2">
                                    <Label htmlFor="dialog-telefono-cliente" className="flex items-center gap-1">
                                        Teléfono <span className="text-red-500">*</span>
                                    </Label>
                                    <Input
                                        id="dialog-telefono-cliente"
                                        name="telefono_cliente"
                                        value={localCliente.telefono_cliente}
                                        onChange={handleLocalChange}
                                        placeholder="Ej: 555-1234"
                                        className={localErrors.telefono_cliente ? 'border-red-500' : ''}
                                    />
                                    {localErrors.telefono_cliente && <p className="text-sm text-red-500">{localErrors.telefono_cliente}</p>}
                                </div>

                                <div className="grid grid-cols-2 gap-4">
                                    <div className="space-y-2">
                                        <Label htmlFor="dialog-ciudad-cliente">Ciudad</Label>
                                        <Input
                                            id="dialog-ciudad-cliente"
                                            name="ciudad_cliente"
                                            value={localCliente.ciudad_cliente}
                                            onChange={handleLocalChange}
                                            placeholder="Ej: MANZANILLO, QUIVICÁN"
                                        />
                                    </div>

                                    <div className="space-y-2">
                                        <Label htmlFor="deuda_inicial">Crédito Inicial</Label>
                                        <div className="flex items-center rounded-md border">
                                            <span className="bg-muted px-3 py-2 text-amber-500">$</span>
                                            <Input type="number" value="0" disabled className="border-0 bg-transparent" />
                                        </div>
                                        <p className="text-xs text-gray-500">Todos los clientes nuevos empiezan con crédito 0</p>
                                    </div>
                                </div>

                                <div className="space-y-2">
                                    <Label htmlFor="dialog-direccion-cliente">Dirección</Label>
                                    <textarea
                                        id="dialog-direccion-cliente"
                                        name="direccion_cliente"
                                        value={localCliente.direccion_cliente}
                                        onChange={handleLocalChange}
                                        placeholder="DIRECCIÓN COMPLETA"
                                        rows={3}
                                        className="border-input ring-offset-background placeholder:text-muted-foreground focus-visible:ring-ring flex w-full rounded-md border bg-transparent px-3 py-2 text-sm focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-50"
                                    />
                                </div>
                            </div>

                            <div className="rounded-lg bg-blue-50 p-4 dark:bg-blue-950/30">
                                <div className="flex items-start gap-3">
                                    <Info className="h-5 w-5 text-blue-600" />
                                    <div className="text-sm text-blue-800 dark:text-blue-300">
                                        <p className="font-medium">¿Cómo funciona el financiamiento con clientes?</p>
                                        <p className="mt-1">
                                            Los clientes pueden prestar dinero a la empresa. Al usar un cliente para pagar una compra, se reduce su
                                            crédito (si tenía) o se genera una nueva deuda con el cliente.
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </ScrollArea>

                <DialogFooter className="gap-2 border-t px-6 py-4">
                    <Button type="button" variant="destructive" onClick={resetDialog}>
                        Cancelar
                    </Button>
                    <Button
                        type="button"
                        onClick={crearClienteLocal}
                        className="bg-sky-600 hover:bg-sky-700"
                        disabled={!localCliente.nombre_cliente.trim() || !localCliente.telefono_cliente.trim()}
                    >
                        <CheckCircle className="mr-2 h-4 w-4" />
                        Crear Cliente
                    </Button>
                </DialogFooter>
            </DialogContent>
        );
    };

    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <Head title="Comprar" />

            {loading && (
                <div className="bg-opacity-50 fixed inset-0 z-50 flex items-center justify-center bg-neutral-800">
                    <Spinner className="text-sidebar-accent size-8" />
                </div>
            )}

            <div className="animate__animated animate__fadeIn flex h-full flex-1 flex-col gap-4 rounded-xl p-4">
                <div className="bg-sidebar border-sidebar-accent animate__animated animate__fadeIn relative col-span-4 space-y-1 overflow-hidden rounded-2xl border border-dashed p-4">
                    <HeadingSmall
                        title="Opciones Generales del Sistema"
                        description="Comprar o adquirir nuevos productos para el negocio, antes de distribuir"
                    />
                    <ShoppingBasket
                        size={70}
                        color="#f59e0b"
                        className="pointer-events-none absolute right-2 bottom-0 translate-x-0 translate-y-[-5] transform animate-pulse opacity-40"
                    />
                </div>

                <Separator className="col-span-4" />

                {/* Sección de Datos Generales de la Compra */}
                <Card className="overflow-hidden border-0 pt-0 shadow-lg">
                    <CardHeader className="bg-gradient-to-r from-indigo-600 to-indigo-700 px-6 py-5 text-white">
                        <div className="flex items-center gap-3">
                            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                                <CalendarIcon className="h-5 w-5" />
                            </div>
                            <div>
                                <CardTitle className="text-base font-semibold text-white">Nuevos Productos</CardTitle>
                                <CardDescription className="text-xs text-indigo-100">
                                    A continuación va a realizar una compra de productos, recuerde asignar: fecha y asociado.
                                </CardDescription>
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent className="pt-5">
                        <form onSubmit={(e) => e.preventDefault()}>
                            <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
                                {/* Fecha de la Compra */}
                                <div className="space-y-2">
                                    <Label htmlFor="fechaCompra" className="text-sm font-medium">
                                        Fecha de la Compra
                                    </Label>
                                    <Popover>
                                        <PopoverTrigger asChild>
                                            <Button
                                                variant={'outline'}
                                                className={cn('h-11 w-full justify-start text-left font-normal', !date && 'text-muted-foreground')}
                                            >
                                                <CalendarIcon className="mr-2 h-4 w-4" />
                                                {date ? format(date, 'PPP') : <span>Seleccione Fecha</span>}
                                            </Button>
                                        </PopoverTrigger>
                                        <PopoverContent className="w-auto p-0" align="start">
                                            <Calendar mode="single" selected={date} onSelect={setDate} initialFocus />
                                        </PopoverContent>
                                    </Popover>
                                    {errors.fecha && <InputError message={errors.fecha} />}
                                </div>

                                {/* Cliente al que se le compra */}
                                <div className="space-y-2">
                                    <Label htmlFor="proveedor" className="text-sm font-medium">
                                        Asociado
                                    </Label>
                                    <SelectorEntidad
                                        id="proveedor"
                                        entidades={entidadesCliente}
                                        valor={clienteDeCompra ? String(clienteDeCompra.id) : ''}
                                        onChange={(entidad) => {
                                            setData((prev) => ({
                                                ...prev,
                                                proveedor: entidad?.nombre ?? '',
                                                // El asociado de la compra no puede además financiarla
                                                pagos_clientes: prev.pagos_clientes.filter((p) => String(p.cliente_id) !== entidad?.id),
                                            }));
                                        }}
                                        placeholder="Buscar asociado..."
                                        pie={
                                            <>
                                                <Separator className="my-2" />
                                                <div
                                                    className="hover:bg-accent flex cursor-pointer items-center gap-2 p-2 text-sm text-blue-600"
                                                    onClick={() => {
                                                        setCrearComoClienteDeCompra(true);
                                                        setIsCrearClienteDialogOpen(true);
                                                    }}
                                                >
                                                    <PlusCircle className="h-4 w-4" />
                                                    Crear Nuevo Asociado
                                                </div>
                                            </>
                                        }
                                    />
                                    {errors.proveedor && <InputError message={errors.proveedor} />}
                                </div>
                            </div>
                        </form>
                    </CardContent>
                </Card>

                {/* Sección de Ingreso de Producto */}
                <Card className="overflow-hidden border-0 pt-0 shadow-lg">
                    <CardHeader className="bg-gradient-to-r from-cyan-600 to-cyan-700 px-6 py-5 text-white">
                        <div className="flex items-center gap-3">
                            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                                <PlusCircle className="h-5 w-5" />
                            </div>
                            <div>
                                <CardTitle className="text-base font-semibold text-white">Ingrese Datos del Producto a Comprar</CardTitle>
                                <CardDescription className="text-xs text-cyan-100">
                                    Identificación, destino y datos comerciales de cada producto
                                </CardDescription>
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent className="space-y-6 pt-5">
                        <FieldSet>
                            <FieldLegend>Identificación del Producto</FieldLegend>
                            <FieldGroup className="grid grid-cols-1 gap-4 md:grid-cols-3">
                                <Field className="relative">
                                    <FieldLabel htmlFor="nombre_producto">Nombre del Producto *</FieldLabel>
                                    <Input
                                        type="text"
                                        name="producto"
                                        placeholder="Producto"
                                        autoComplete="off"
                                        value={tempFormData.producto}
                                        onChange={handleTempInputChange}
                                        onFocus={() => setCampoEnFoco('producto')}
                                        onBlur={() => setTimeout(() => setCampoEnFoco((c) => (c === 'producto' ? null : c)), 150)}
                                        onKeyDown={(e) => handleAutocompleteKeyDown(e, 'producto')}
                                    />
                                    <ProductoSugerenciasDropdown
                                        mostrar={campoEnFoco === 'producto' && tempFormData.producto.trim().length >= 2}
                                        buscando={buscandoProducto}
                                        sugerencias={productoSugerencias}
                                        resaltada={sugerenciaResaltada}
                                        onResaltar={setSugerenciaResaltada}
                                        onSeleccionar={seleccionarProductoExistente}
                                    />
                                </Field>

                                <Field className="relative">
                                    <FieldLabel htmlFor="marca_producto">Marca</FieldLabel>
                                    <Input
                                        type="text"
                                        name="marca"
                                        placeholder="Marca"
                                        autoComplete="off"
                                        value={tempFormData.marca}
                                        onChange={handleTempInputChange}
                                        onFocus={() => setCampoEnFoco('marca')}
                                        onBlur={() => setTimeout(() => setCampoEnFoco((c) => (c === 'marca' ? null : c)), 150)}
                                        onKeyDown={(e) => handleAutocompleteKeyDown(e, 'marca')}
                                    />
                                    <ProductoSugerenciasDropdown
                                        mostrar={campoEnFoco === 'marca' && tempFormData.marca.trim().length >= 2}
                                        buscando={buscandoProducto}
                                        sugerencias={productoSugerencias}
                                        resaltada={sugerenciaResaltada}
                                        onResaltar={setSugerenciaResaltada}
                                        onSeleccionar={seleccionarProductoExistente}
                                    />
                                </Field>

                                <Field className="relative">
                                    <FieldLabel htmlFor="modelo_producto">Modelo</FieldLabel>
                                    <Input
                                        type="text"
                                        name="modelo"
                                        placeholder="Modelo"
                                        autoComplete="off"
                                        value={tempFormData.modelo}
                                        onChange={handleTempInputChange}
                                        onFocus={() => setCampoEnFoco('modelo')}
                                        onBlur={() => setTimeout(() => setCampoEnFoco((c) => (c === 'modelo' ? null : c)), 150)}
                                        onKeyDown={(e) => handleAutocompleteKeyDown(e, 'modelo')}
                                    />
                                    <ProductoSugerenciasDropdown
                                        mostrar={campoEnFoco === 'modelo' && tempFormData.modelo.trim().length >= 2}
                                        buscando={buscandoProducto}
                                        sugerencias={productoSugerencias}
                                        resaltada={sugerenciaResaltada}
                                        onResaltar={setSugerenciaResaltada}
                                        onSeleccionar={seleccionarProductoExistente}
                                    />
                                </Field>

                                <Field>
                                    <FieldLabel htmlFor="capacidad_producto">Capacidad/Tamaño</FieldLabel>
                                    <Input
                                        type="text"
                                        name="capacidad"
                                        placeholder="Ej: 1TB, 16GB"
                                        value={tempFormData.capacidad}
                                        onChange={handleTempInputChange}
                                        onKeyDown={handleEnterAgregarProducto}
                                    />
                                </Field>

                                <Field>
                                    <FieldLabel htmlFor="color_producto">Color</FieldLabel>
                                    <Input
                                        type="text"
                                        name="color"
                                        placeholder="Ej: Negro, Rojo, Azul"
                                        value={tempFormData.color}
                                        onChange={handleTempInputChange}
                                        onKeyDown={handleEnterAgregarProducto}
                                    />
                                </Field>

                                <Field>
                                    <FieldLabel htmlFor="codigo_preview">Código (autogenerado)</FieldLabel>
                                    <Input
                                        id="codigo_preview"
                                        value={codigoPreview}
                                        placeholder="Se genera al completar el nombre"
                                        disabled
                                        className="font-mono text-xs"
                                    />
                                </Field>
                            </FieldGroup>
                        </FieldSet>

                        <FieldSeparator />

                        <FieldSet>
                            <FieldLegend>Destino y Categoría</FieldLegend>
                            <FieldGroup className="grid grid-cols-1 gap-4 md:grid-cols-2">
                                <Field>
                                    <FieldLabel htmlFor="almacen_id">Almacén Destino *</FieldLabel>
                                    <Combobox
                                        items={almacens}
                                        itemToStringLabel={(item) => item.nombre_almacen}
                                        itemToStringValue={(item) => item.nombre_almacen}
                                        value={selectedAlmacen}
                                        onValueChange={(almacen) => {
                                            if (almacen) {
                                                handleTempSelectChange('almacen_id', almacen.id.toString());
                                            } else {
                                                handleTempSelectChange('almacen_id', '');
                                            }
                                        }}
                                    >
                                        <ComboboxInput
                                            placeholder={lastSelectedAlmacenId ? 'Último seleccionado' : 'Seleccione Almacén'}
                                            showClear={!!tempFormData.almacen_id}
                                            className="uppercase"
                                        />
                                        <ComboboxContent>
                                            <ComboboxEmpty>No se encontraron almacenes.</ComboboxEmpty>
                                            <ComboboxList>
                                                {(almacen) => (
                                                    <ComboboxItem key={almacen.id} value={almacen}>
                                                        <div className="flex items-center gap-2 uppercase">
                                                            <Warehouse className="h-4 w-4 text-gray-500" />
                                                            <span>{almacen.nombre_almacen}</span>
                                                            {almacen.tipo_almacen && (
                                                                <Badge variant="outline" className="ml-auto text-xs">
                                                                    {almacen.tipo_almacen}
                                                                </Badge>
                                                            )}
                                                        </div>
                                                    </ComboboxItem>
                                                )}
                                            </ComboboxList>
                                            <Separator className="my-2" />
                                            <div
                                                className="flex cursor-pointer items-center rounded-md bg-blue-50 px-3 py-3 text-blue-700 hover:bg-blue-100 dark:bg-blue-950/30 dark:text-blue-300"
                                                onClick={() => {
                                                    setIsCrearAlmacenDialogOpen(true);
                                                }}
                                            >
                                                <PlusCircle className="mr-2 h-4 w-4" />
                                                <span className="font-medium">Crear nuevo almacén</span>
                                            </div>
                                        </ComboboxContent>
                                    </Combobox>
                                    {!tempFormData.almacen_id && productos.some((p) => !p.almacen_id) && (
                                        <p className="text-sm text-red-500">Debe seleccionar un almacén válido</p>
                                    )}
                                </Field>

                                <Field>
                                    <FieldLabel htmlFor="categorias">Categoría *</FieldLabel>
                                    <Combobox
                                        items={categorias}
                                        itemToStringLabel={(item) => item.nombre_categoria}
                                        itemToStringValue={(item) => item.nombre_categoria}
                                        value={selectedCategoria}
                                        onValueChange={(categoria) => {
                                            if (categoria) {
                                                handleTempSelectChange('categoria', categoria.nombre_categoria);
                                            } else {
                                                handleTempSelectChange('categoria', '');
                                            }
                                        }}
                                    >
                                        <ComboboxInput
                                            placeholder="Seleccione Categoría"
                                            showClear={!!tempFormData.categoria}
                                            className="uppercase"
                                        />
                                        <ComboboxContent>
                                            <ComboboxEmpty>No se encontraron categorías.</ComboboxEmpty>
                                            <ComboboxList>
                                                {(categoria) => (
                                                    <ComboboxItem key={categoria.id} value={categoria}>
                                                        <span className="uppercase">{categoria.nombre_categoria}</span>
                                                    </ComboboxItem>
                                                )}
                                            </ComboboxList>
                                            <Separator className="my-2" />
                                            <div
                                                className="hover:bg-accent flex cursor-pointer items-center gap-2 p-2 text-sm text-purple-600"
                                                onClick={() => setIsCrearCategoriaDialogOpen(true)}
                                            >
                                                <PlusCircle className="h-4 w-4" />
                                                Crear Nueva Categoría
                                            </div>
                                        </ComboboxContent>
                                    </Combobox>
                                    {errors.categorias && <InputError message={errors.categorias} />}
                                </Field>
                            </FieldGroup>
                        </FieldSet>

                        <FieldSeparator />

                        <FieldSet>
                            <FieldLegend>Datos Comerciales</FieldLegend>
                            <FieldGroup className="grid grid-cols-1 gap-4 md:grid-cols-2">
                                <Field>
                                    <FieldLabel htmlFor="precio_producto">Precio *</FieldLabel>
                                    <InputGroup>
                                        <InputGroupAddon>$</InputGroupAddon>
                                        <InputGroupInput
                                            inputMode="decimal"
                                            name="precio"
                                            placeholder="0.00"
                                            value={tempFormData.precio ?? ''}
                                            onChange={handleTempInputChange}
                                            onKeyDown={handleEnterAgregarProducto}
                                        />
                                    </InputGroup>
                                </Field>

                                <Field>
                                    <FieldLabel htmlFor="cantidad_producto">Cantidad *</FieldLabel>
                                    <Input
                                        inputMode="numeric"
                                        name="cantidad"
                                        placeholder="0"
                                        value={tempFormData.cantidad ?? ''}
                                        onChange={handleTempInputChange}
                                        onKeyDown={handleEnterAgregarProducto}
                                    />
                                </Field>
                            </FieldGroup>

                            {productoCoincidente &&
                                (() => {
                                    // El stock es por almacén; el costo (precio_compra_producto) es la última
                                    // referencia conocida, no un solo dato fijo por producto. Desde 2026-09-20,
                                    // CompraController::procesarLineasProducto() reusa la ficha existente cuando
                                    // nombre+categoría+marca+modelo+capacidad coinciden EXACTO (mismo criterio que
                                    // Producto::fichasHermanas()) — esta compra pasa a ser un lote nuevo bajo esa
                                    // misma ficha, con su propio costo real por almacén (ver
                                    // Producto::costoEnAlmacen()), en vez de duplicar el catálogo. Si algún campo
                                    // no coincide exacto (aunque la búsqueda de arriba haya encontrado la
                                    // sugerencia por coincidencia parcial), sí crea una ficha separada.
                                    const stockEnEsteAlmacen = selectedAlmacen
                                        ? (productoCoincidente.stock_por_almacen.find((s) => s.almacen_id === selectedAlmacen.id)?.cantidad ?? 0)
                                        : null;

                                    const esIdentidadExacta =
                                        productoCoincidente.nombre_producto === tempFormData.producto.trim() &&
                                        (productoCoincidente.marca_producto || '') === tempFormData.marca.trim() &&
                                        (productoCoincidente.modelo_producto || '') === tempFormData.modelo.trim() &&
                                        (productoCoincidente.capacidad_producto || '') === tempFormData.capacidad.trim() &&
                                        (productoCoincidente.categoria || '') === tempFormData.categoria.trim();

                                    return (
                                        <div className="mt-4 flex items-start gap-3 rounded-lg border border-amber-200 bg-amber-50 p-3 text-sm dark:border-amber-800 dark:bg-amber-950/30">
                                            <Info className="mt-0.5 h-4 w-4 shrink-0 text-amber-600 dark:text-amber-400" />
                                            <p className="text-amber-800 dark:text-amber-300">
                                                Ya existe un producto con estos datos.{' '}
                                                {stockEnEsteAlmacen !== null ? (
                                                    <>
                                                        Tiene <strong>{stockEnEsteAlmacen}</strong> unidades en{' '}
                                                        <strong>{selectedAlmacen!.nombre_almacen}</strong>
                                                        {productoCoincidente.cantidad_total > stockEnEsteAlmacen && (
                                                            <> ({productoCoincidente.cantidad_total} en total entre todos los almacenes)</>
                                                        )}
                                                        .{' '}
                                                    </>
                                                ) : (
                                                    <>
                                                        Tiene <strong>{productoCoincidente.cantidad_total}</strong> unidades en stock, repartidas
                                                        entre los almacenes que ya lo tienen — elegí el Almacén Destino para ver cuánto hay ahí
                                                        específicamente.{' '}
                                                    </>
                                                )}
                                                Su costo actual es <strong>${productoCoincidente.precio_compra_producto.toFixed(2)}</strong>.{' '}
                                                {esIdentidadExacta ? (
                                                    <>
                                                        Esta compra va a sumar un <strong>lote nuevo</strong> a esta misma ficha (no un duplicado) —
                                                        con su propio costo real en el almacén que elijas, sin tocar los demás.
                                                    </>
                                                ) : (
                                                    <>
                                                        Marca, modelo, capacidad o categoría no coinciden exacto con esta ficha, así que esta compra
                                                        va a registrar una ficha nueva y separada. Si terminan siendo el mismo artículo, se pueden
                                                        unificar después desde la herramienta de fusión de duplicados en Productos.
                                                    </>
                                                )}
                                            </p>
                                        </div>
                                    );
                                })()}
                        </FieldSet>

                        {/* Botón Agregar */}
                        <div className="mt-6 flex justify-end">
                            <Button variant="secondary" className="cursor-pointer hover:animate-pulse hover:bg-blue-400" onClick={agregarProducto}>
                                <PlusIcon />
                                Agregar Producto
                            </Button>
                        </div>
                    </CardContent>
                </Card>

                {/* Sección de la Tabla de Productos CON DIÁLOGO MEJORADO */}
                <Card className="overflow-hidden border-0 pt-0 shadow-lg">
                    <CardHeader className="bg-gradient-to-r from-teal-600 to-teal-700 px-6 py-5 text-white">
                        <div className="flex items-center gap-3">
                            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                                <Package className="h-5 w-5" />
                            </div>
                            <div className="flex flex-1 items-center justify-between">
                                <CardTitle className="text-base font-semibold text-white">Lista de los Productos a Comprar</CardTitle>
                                <Badge variant="outline" className="border-white/30 bg-white/20 text-white backdrop-blur-sm">
                                    {productos.length} {productos.length === 1 ? 'producto' : 'productos'}
                                </Badge>
                            </div>
                        </div>
                    </CardHeader>
                    <CardContent className="p-0">
                        <Table>
                            <TableHeader>
                                <TableRow className="bg-sidebar-accent hover:bg-sidebar-accent">
                                    <TableHead>Producto</TableHead>
                                    <TableHead>Marca</TableHead>
                                    <TableHead>Modelo</TableHead>
                                    <TableHead>Capacidad</TableHead>
                                    <TableHead>Color</TableHead>
                                    <TableHead>Almacén</TableHead>
                                    <TableHead>Categoria</TableHead>
                                    <TableHead>Código</TableHead>
                                    <TableHead>Cant.</TableHead>
                                    <TableHead>Precio</TableHead>
                                    <TableHead>Importe</TableHead>
                                    <TableHead className="text-right">Acciones</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody className="[&>tr:nth-child(even)]:bg-muted/40">
                                {productos.length === 0 && (
                                    <TableRow className="hover:bg-transparent">
                                        <TableCell colSpan={12} className="text-muted-foreground h-24 text-center">
                                            Todavía no agregaste productos a esta compra.
                                        </TableCell>
                                    </TableRow>
                                )}
                                {productos.map((p) => {
                                    const almacen = almacens.find((a) => a.id === p.almacen_id);
                                    return (
                                        <TableRow key={p.id} className={!almacen ? 'bg-red-50 dark:bg-red-950/20' : ''}>
                                            <TableCell className="font-medium">{p.producto}</TableCell>
                                            <TableCell>{p.marca || 'N/A'}</TableCell>
                                            <TableCell>{p.modelo || 'N/A'}</TableCell>
                                            <TableCell>{p.capacidad || 'N/A'}</TableCell>
                                            <TableCell>{p.color || 'N/A'}</TableCell>
                                            <TableCell>
                                                <div className="flex items-center gap-2">
                                                    {almacen ? (
                                                        <>
                                                            <Warehouse className="h-4 w-4 text-gray-500" />
                                                            <span>{almacen.nombre_almacen}</span>
                                                            {almacen.tipo_almacen && (
                                                                <Badge variant="outline" className="text-xs">
                                                                    {almacen.tipo_almacen}
                                                                </Badge>
                                                            )}
                                                        </>
                                                    ) : (
                                                        <span className="flex items-center gap-2 text-red-600">
                                                            <X className="h-4 w-4" />
                                                            <span>Almacén no válido</span>
                                                        </span>
                                                    )}
                                                </div>
                                            </TableCell>
                                            <TableCell>{p.categoria}</TableCell>
                                            <TableCell>{p.codigo || 'ERROR'}</TableCell>
                                            <TableCell>{p.cantidad}</TableCell>
                                            <TableCell>${p.precio.toFixed(2)}</TableCell>
                                            <TableCell>${(p.cantidad * p.precio).toFixed(2)}</TableCell>
                                            <TableCell className="text-right">
                                                {/* DIÁLOGO MEJORADO - LISTO PARA USAR */}
                                                <AlertDialog open={isDialogOpen && editingProductId === p.id} onOpenChange={setIsDialogOpen}>
                                                    <AlertDialogTrigger asChild>
                                                        <Button
                                                            variant="link"
                                                            className="cursor-pointer text-blue-600 hover:text-blue-800"
                                                            onClick={() => editarProducto(p.id)}
                                                        >
                                                            <Edit2 />
                                                        </Button>
                                                    </AlertDialogTrigger>

                                                    <AlertDialogContent className="flex h-[90vh] w-[95vw] !max-w-none max-w-[1024px] flex-col p-0">
                                                        <AlertDialogHeader className="shrink-0 border-b bg-gradient-to-r from-fuchsia-600 to-fuchsia-700 px-6 py-5 text-white">
                                                            <div className="flex items-center gap-3">
                                                                <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                                                                    <Edit2 className="h-5 w-5" />
                                                                </div>
                                                                <div>
                                                                    <AlertDialogTitle className="text-xl text-white sm:text-2xl">
                                                                        Editar Producto
                                                                    </AlertDialogTitle>
                                                                    <AlertDialogDescription className="text-fuchsia-100">
                                                                        Realiza ajustes detallados al producto. Los cambios se reflejarán en la
                                                                        lista de compra.
                                                                    </AlertDialogDescription>
                                                                </div>
                                                            </div>
                                                        </AlertDialogHeader>

                                                        <div className="grid flex-1 overflow-hidden lg:grid-cols-[1fr_380px]">
                                                            {/* CONTENIDO PRINCIPAL */}
                                                            <div className="flex flex-col gap-y-8 overflow-y-auto px-6 py-8">
                                                                <div className="space-y-6 rounded-lg border border-slate-200 p-6 dark:border-slate-700">
                                                                    <h3 className="text-lg font-semibold text-gray-800 dark:text-gray-200">
                                                                        Detalles del Producto
                                                                    </h3>

                                                                    <div className="grid grid-cols-1 gap-x-6 gap-y-4 sm:grid-cols-2">
                                                                        <div className="sm:col-span-2">
                                                                            <Label htmlFor="edit-producto">Nombre del Producto *</Label>
                                                                            <Input
                                                                                id="edit-producto"
                                                                                name="producto"
                                                                                value={tempFormData.producto}
                                                                                onChange={handleTempInputChange}
                                                                            />
                                                                        </div>

                                                                        <div>
                                                                            <Label htmlFor="edit-marca">Marca</Label>
                                                                            <Input
                                                                                id="edit-marca"
                                                                                name="marca"
                                                                                value={tempFormData.marca}
                                                                                onChange={handleTempInputChange}
                                                                            />
                                                                        </div>

                                                                        <div>
                                                                            <Label htmlFor="edit-modelo">Modelo</Label>
                                                                            <Input
                                                                                id="edit-modelo"
                                                                                name="modelo"
                                                                                value={tempFormData.modelo}
                                                                                onChange={handleTempInputChange}
                                                                            />
                                                                        </div>

                                                                        <div className="sm:col-span-2">
                                                                            <Label htmlFor="edit-capacidad">Capacidad / Tamaño</Label>
                                                                            <Input
                                                                                id="edit-capacidad"
                                                                                name="capacidad"
                                                                                value={tempFormData.capacidad}
                                                                                onChange={handleTempInputChange}
                                                                            />
                                                                        </div>

                                                                        <div className="sm:col-span-2">
                                                                            <Label htmlFor="edit-color">Color</Label>
                                                                            <Input
                                                                                id="edit-color"
                                                                                name="color"
                                                                                placeholder="Ej: Negro, Rojo, Azul"
                                                                                value={tempFormData.color}
                                                                                onChange={handleTempInputChange}
                                                                            />
                                                                        </div>
                                                                    </div>
                                                                </div>

                                                                <div className="space-y-6 rounded-lg border border-slate-200 p-6 dark:border-slate-700">
                                                                    <h3 className="text-lg font-semibold text-gray-800 dark:text-gray-200">
                                                                        Inventario y Precio
                                                                    </h3>

                                                                    <div className="grid grid-cols-1 gap-x-6 gap-y-4 sm:grid-cols-2">
                                                                        <div>
                                                                            <Label htmlFor="edit-cantidad">Cantidad *</Label>
                                                                            <Input
                                                                                id="edit-cantidad"
                                                                                type="number"
                                                                                name="cantidad"
                                                                                value={tempFormData.cantidad ?? ''}
                                                                                onChange={handleTempInputChange}
                                                                                min="1"
                                                                            />
                                                                        </div>

                                                                        <div>
                                                                            <Label htmlFor="edit-precio">Precio Unitario *</Label>
                                                                            <div className="relative">
                                                                                <span className="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3 text-gray-500">
                                                                                    $
                                                                                </span>
                                                                                <Input
                                                                                    id="edit-precio"
                                                                                    type="number"
                                                                                    step="0.01"
                                                                                    name="precio"
                                                                                    value={tempFormData.precio ?? ''}
                                                                                    onChange={handleTempInputChange}
                                                                                    className="pl-7"
                                                                                    min="0.01"
                                                                                />
                                                                            </div>
                                                                        </div>
                                                                    </div>
                                                                </div>
                                                            </div>

                                                            {/* SIDEBAR */}
                                                            <div
                                                                ref={resolveEditDialogContainer}
                                                                className="flex flex-col border-l border-slate-200 bg-slate-50/50 p-6 dark:border-slate-700 dark:bg-slate-800/20"
                                                            >
                                                                <div className="space-y-6">
                                                                    <h3 className="text-lg font-semibold text-gray-800 dark:text-gray-200">
                                                                        Organización
                                                                    </h3>

                                                                    <div>
                                                                        <Label>Almacén Destino *</Label>
                                                                        <Combobox
                                                                            items={almacens}
                                                                            itemToStringLabel={(item) => item.nombre_almacen}
                                                                            itemToStringValue={(item) => item.nombre_almacen}
                                                                            value={selectedAlmacen}
                                                                            onValueChange={(almacen) => {
                                                                                if (almacen) {
                                                                                    handleTempSelectChange('almacen_id', almacen.id.toString());
                                                                                } else {
                                                                                    handleTempSelectChange('almacen_id', '');
                                                                                }
                                                                            }}
                                                                        >
                                                                            <ComboboxInput
                                                                                placeholder="Seleccione Almacén"
                                                                                showClear={!!tempFormData.almacen_id}
                                                                                className="uppercase"
                                                                            />
                                                                            <ComboboxContent container={editDialogContainer}>
                                                                                <ComboboxEmpty>No se encontraron almacenes.</ComboboxEmpty>
                                                                                <ComboboxList>
                                                                                    {(almacen) => (
                                                                                        <ComboboxItem key={almacen.id} value={almacen}>
                                                                                            <span className="uppercase">
                                                                                                {almacen.nombre_almacen}
                                                                                            </span>
                                                                                        </ComboboxItem>
                                                                                    )}
                                                                                </ComboboxList>
                                                                            </ComboboxContent>
                                                                        </Combobox>
                                                                    </div>

                                                                    <div>
                                                                        <Label>Categoría *</Label>
                                                                        <Combobox
                                                                            items={categorias}
                                                                            itemToStringLabel={(item) => item.nombre_categoria}
                                                                            itemToStringValue={(item) => item.nombre_categoria}
                                                                            value={selectedCategoria}
                                                                            onValueChange={(categoria) => {
                                                                                if (categoria) {
                                                                                    handleTempSelectChange('categoria', categoria.nombre_categoria);
                                                                                } else {
                                                                                    handleTempSelectChange('categoria', '');
                                                                                }
                                                                            }}
                                                                        >
                                                                            <ComboboxInput
                                                                                placeholder="Seleccione o cree"
                                                                                showClear={!!tempFormData.categoria}
                                                                                className="uppercase"
                                                                            />
                                                                            <ComboboxContent container={editDialogContainer}>
                                                                                <ComboboxEmpty>No se encontraron categorías.</ComboboxEmpty>
                                                                                <ComboboxList>
                                                                                    {(categoria) => (
                                                                                        <ComboboxItem key={categoria.id} value={categoria}>
                                                                                            {categoria.nombre_categoria}
                                                                                        </ComboboxItem>
                                                                                    )}
                                                                                </ComboboxList>
                                                                            </ComboboxContent>
                                                                        </Combobox>
                                                                    </div>

                                                                    <div>
                                                                        <Label>Código de Barras</Label>
                                                                        <Input value={tempFormData.codigo || ''} disabled />
                                                                    </div>
                                                                </div>

                                                                <div className="mt-auto rounded-xl border-2 border-blue-500/40 bg-blue-50/50 p-4 dark:bg-blue-900/30">
                                                                    <div className="flex justify-between">
                                                                        <span>Subtotal</span>
                                                                        <span className="font-bold">
                                                                            ${((tempFormData.cantidad || 0) * (tempFormData.precio || 0)).toFixed(2)}
                                                                        </span>
                                                                    </div>
                                                                </div>
                                                            </div>
                                                        </div>

                                                        <AlertDialogFooter className="shrink-0 flex-row justify-end space-x-4 border-t px-6 py-4">
                                                            <AlertDialogCancel asChild>
                                                                <Button variant="ghost" onClick={() => setIsDialogOpen(false)}>
                                                                    Cancelar
                                                                </Button>
                                                            </AlertDialogCancel>

                                                            <Button
                                                                className="bg-fuchsia-600 text-white hover:bg-fuchsia-700"
                                                                onClick={handleActualizarProducto}
                                                            >
                                                                <HardDriveUpload className="mr-2 h-4 w-4" />
                                                                Guardar Cambios
                                                            </Button>
                                                        </AlertDialogFooter>
                                                    </AlertDialogContent>
                                                </AlertDialog>

                                                <Button
                                                    variant="link"
                                                    onClick={() => eliminarProducto(p.id)}
                                                    className="ms-2 cursor-pointer text-red-600 hover:text-red-800"
                                                >
                                                    <Trash2Icon />
                                                </Button>
                                            </TableCell>
                                        </TableRow>
                                    );
                                })}
                            </TableBody>
                            <TableFooter>
                                <TableRow className="hover:bg-transparent">
                                    <TableCell colSpan={6} className="font-medium">
                                        {productos.length} {productos.length === 1 ? 'tipo de mercancía' : 'tipos de mercancía'}
                                    </TableCell>
                                    <TableCell className="font-medium">{productos.reduce((t, p) => t + p.cantidad, 0)} unidades</TableCell>
                                    <TableCell colSpan={5} className="text-right text-base font-bold text-emerald-600 dark:text-emerald-400">
                                        Importe General: ${calcularTotal()}
                                    </TableCell>
                                </TableRow>
                            </TableFooter>
                        </Table>
                    </CardContent>
                </Card>

                {/* ==================== PROCESAR LAS COMPRAS DE LOS PRODUCTOS ==================== */}
                <div className="flex justify-center gap-4 p-6">
                    <AlertDialog>
                        <AlertDialogTrigger asChild>
                            <Button className="bg-sidebar-accent cursor-pointer text-white hover:bg-emerald-500">
                                <ShoppingCart className="mr-2 h-5 w-5" />
                                Realizar Compra
                                {productos.some((p) => !almacens.find((a) => a.id === p.almacen_id)) && (
                                    <Badge variant="destructive" className="ml-2">
                                        Almacén inválido
                                    </Badge>
                                )}
                            </Button>
                        </AlertDialogTrigger>

                        <AlertDialogContent className="flex max-h-[90vh] flex-col overflow-hidden p-0 sm:max-w-5xl">
                            {(() => {
                                const step = data.compra === 'deuda_proveedor' ? 'deuda' : data.compra === 'pago_cash' ? 'pago' : 'select';

                                // Totales del paso "Procesar Pago" — calculados una sola vez y reutilizados en el
                                // header (barra de progreso + chips) y en el footer (botón Confirmar Pago).
                                const totalCuentas = data.pagos.reduce((a, p) => a + p.monto, 0);
                                const totalClientes = data.pagos_clientes.reduce((a, p) => a + (p.monto || 0), 0);
                                const totalCubierto = totalCuentas + totalClientes;
                                const totalCompra = parseFloat(calcularTotal());
                                const restante = Math.max(0, totalCompra - totalCubierto);
                                const progresoPct = totalCompra > 0 ? Math.min(100, (totalCubierto / totalCompra) * 100) : 0;
                                const pagoCompleto = Number(totalCubierto.toFixed(2)) === Number(totalCompra.toFixed(2));
                                // Pagar de más nunca se permite, con o sin deuda parcial habilitada — mismo
                                // criterio que el backend.
                                const sePaso = totalCubierto - totalCompra > 0.01;
                                const puedeConfirmar = !sePaso && (pagoCompleto || (data.permitir_deuda_parcial && totalCubierto > 0));

                                return (
                                    <>
                                        {/* STEP 1: SELECTOR DE TIPO */}
                                        {step === 'select' && (
                                            <>
                                                <AlertDialogHeader className="border-b bg-gradient-to-r from-emerald-600 to-emerald-700 px-8 py-6 text-white">
                                                    <div className="flex items-center gap-4">
                                                        <div className="flex h-14 w-14 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                                                            <ShoppingCart className="h-8 w-8" />
                                                        </div>
                                                        <div>
                                                            <AlertDialogTitle className="text-2xl font-bold">Finalizar Compra</AlertDialogTitle>
                                                            <AlertDialogDescription className="text-emerald-100">
                                                                Total a pagar:{' '}
                                                                <span className="text-3xl font-bold">${parseFloat(calcularTotal()).toFixed(2)}</span>
                                                            </AlertDialogDescription>
                                                        </div>
                                                    </div>
                                                </AlertDialogHeader>

                                                <div className="px-8 py-12">
                                                    <h3 className="mb-8 text-center text-xl font-semibold">¿Cómo deseas registrar esta compra?</h3>
                                                    <div className="grid gap-6 sm:grid-cols-2">
                                                        <button
                                                            onClick={() => setData('compra', 'deuda_proveedor')}
                                                            className="flex cursor-pointer flex-col gap-4 rounded-xl border-2 border-gray-200 p-6 text-left transition-all hover:border-orange-500 hover:bg-orange-50 dark:border-gray-700 dark:hover:bg-orange-950/20"
                                                        >
                                                            <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-orange-100 dark:bg-orange-950">
                                                                <CreditCard className="h-6 w-6 text-orange-600" />
                                                            </div>
                                                            <div>
                                                                <p className="font-semibold">Generar Deuda</p>
                                                                <p className="text-muted-foreground text-sm">
                                                                    Registrar compra a crédito al asociado
                                                                </p>
                                                            </div>
                                                        </button>

                                                        <button
                                                            onClick={() => setData('compra', 'pago_cash')}
                                                            className="flex cursor-pointer flex-col gap-4 rounded-xl border-2 border-gray-200 p-6 text-left transition-all hover:border-emerald-500 hover:bg-emerald-50 dark:border-gray-700 dark:hover:bg-emerald-950/20"
                                                        >
                                                            <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-emerald-100 dark:bg-emerald-950">
                                                                <DollarSign className="h-6 w-6 text-emerald-600" />
                                                            </div>
                                                            <div>
                                                                <p className="font-semibold">Pagar Ahora</p>
                                                                <p className="text-muted-foreground text-sm">
                                                                    Realizar pago inmediato con fondos disponibles
                                                                </p>
                                                            </div>
                                                        </button>
                                                    </div>
                                                </div>

                                                <AlertDialogFooter className="bg-muted/50 border-t px-8 py-5">
                                                    <AlertDialogCancel className="bg-sidebar-accent hover:bg-destructive cursor-pointer text-white">
                                                        Cancelar
                                                    </AlertDialogCancel>
                                                </AlertDialogFooter>
                                            </>
                                        )}

                                        {/* STEP 2: CONFIRMAR DEUDA */}
                                        {step === 'deuda' && (
                                            <>
                                                <AlertDialogHeader className="border-b bg-gradient-to-r from-orange-600 to-orange-700 px-8 py-6 text-white">
                                                    <div className="flex items-center gap-4">
                                                        <div className="flex h-14 w-14 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                                                            <CreditCard className="h-8 w-8" />
                                                        </div>
                                                        <div>
                                                            <AlertDialogTitle className="text-2xl font-bold">Registrar Deuda</AlertDialogTitle>
                                                            <AlertDialogDescription className="text-orange-100">
                                                                Confirma la creación de deuda con el asociado
                                                            </AlertDialogDescription>
                                                        </div>
                                                    </div>
                                                </AlertDialogHeader>

                                                <div className="px-8 py-8">
                                                    <div className="rounded-xl border border-orange-200 bg-orange-50 p-6 dark:border-orange-800 dark:bg-orange-950/30">
                                                        <h4 className="mb-4 font-semibold">Resumen de la operación</h4>
                                                        <div className="space-y-3">
                                                            <div className="flex justify-between">
                                                                <span className="text-muted-foreground">Monto total:</span>
                                                                <span className="text-xl font-bold">${parseFloat(calcularTotal()).toFixed(2)}</span>
                                                            </div>
                                                            <Separator />
                                                            <p className="text-muted-foreground text-sm">
                                                                Esta compra se registrará como deuda con el asociado y será pagadera en el futuro.
                                                            </p>
                                                        </div>
                                                    </div>
                                                </div>

                                                <AlertDialogFooter className="bg-muted/50 border-t px-8 py-5">
                                                    <Button
                                                        variant="outline"
                                                        onClick={() => setData('compra', '')}
                                                        className="h-12 cursor-pointer px-6"
                                                    >
                                                        Atrás
                                                    </Button>
                                                    <Button
                                                        onClick={realizarCompra}
                                                        disabled={processing}
                                                        className="h-12 cursor-pointer bg-orange-600 px-8 hover:bg-orange-700"
                                                    >
                                                        {processing ? (
                                                            <>
                                                                Procesando <Loader2 className="ml-2 h-5 w-5 animate-spin" />
                                                            </>
                                                        ) : (
                                                            <>
                                                                Confirmar Deuda <CheckCircle className="ml-2 h-5 w-5" />
                                                            </>
                                                        )}
                                                    </Button>
                                                </AlertDialogFooter>
                                            </>
                                        )}

                                        {/* STEP 3: PAGAR AHORA */}
                                        {step === 'pago' && (
                                            <>
                                                <AlertDialogHeader className="border-b bg-gradient-to-r from-emerald-600 to-emerald-700 px-8 py-6 text-white">
                                                    <div className="flex items-center gap-4">
                                                        <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                                                            <DollarSign className="h-8 w-8" />
                                                        </div>
                                                        <div className="min-w-0 flex-1">
                                                            <AlertDialogTitle className="text-2xl font-bold">Procesar Pago</AlertDialogTitle>
                                                            <AlertDialogDescription className="text-emerald-100">
                                                                Total a cubrir:{' '}
                                                                <span className="text-3xl font-bold text-white">${totalCompra.toFixed(2)}</span>
                                                            </AlertDialogDescription>
                                                        </div>
                                                        <label className="flex shrink-0 cursor-pointer items-center gap-2 self-start rounded-full bg-white/15 px-3 py-1.5 text-xs font-medium text-white">
                                                            <Checkbox
                                                                checked={data.permitir_deuda_parcial}
                                                                onCheckedChange={(checked) => setData('permitir_deuda_parcial', checked === true)}
                                                                className="border-white/40 bg-white/10 data-[state=checked]:border-white data-[state=checked]:bg-white data-[state=checked]:text-emerald-700"
                                                            />
                                                            Permitir completar con deuda si no alcanza
                                                        </label>
                                                    </div>

                                                    <div className="mt-5 space-y-2">
                                                        <div className="h-2 w-full overflow-hidden rounded-full bg-white/20">
                                                            <div
                                                                className={cn(
                                                                    'h-full rounded-full transition-all',
                                                                    pagoCompleto ? 'bg-green-400' : progresoPct >= 34 ? 'bg-yellow-300' : 'bg-red-400',
                                                                )}
                                                                style={{ width: `${progresoPct}%` }}
                                                            />
                                                        </div>
                                                        <div className="flex flex-wrap items-center gap-2 text-sm">
                                                            <span className="flex items-center gap-1.5 rounded-full bg-white/15 px-3 py-1">
                                                                <Wallet className="h-3.5 w-3.5" />
                                                                Cuentas: <span className="font-semibold text-white">${totalCuentas.toFixed(2)}</span>
                                                            </span>
                                                            <span className="flex items-center gap-1.5 rounded-full bg-white/15 px-3 py-1">
                                                                <Users className="h-3.5 w-3.5" />
                                                                Clientes:{' '}
                                                                <span className="font-semibold text-white">${totalClientes.toFixed(2)}</span>
                                                            </span>
                                                            <span
                                                                className={cn(
                                                                    'ml-auto flex items-center gap-1.5 rounded-full px-3 py-1 font-semibold',
                                                                    pagoCompleto
                                                                        ? 'bg-white text-emerald-700'
                                                                        : data.permitir_deuda_parcial
                                                                          ? 'bg-amber-400 text-amber-950'
                                                                          : 'bg-white/15',
                                                                )}
                                                            >
                                                                {pagoCompleto ? (
                                                                    <>
                                                                        <CheckCircle className="h-3.5 w-3.5" /> Cubierto: ${totalCubierto.toFixed(2)}
                                                                    </>
                                                                ) : data.permitir_deuda_parcial ? (
                                                                    <>Quedará como deuda: ${restante.toFixed(2)}</>
                                                                ) : (
                                                                    <>Restante: ${restante.toFixed(2)}</>
                                                                )}
                                                            </span>
                                                        </div>
                                                    </div>
                                                </AlertDialogHeader>

                                                <ScrollArea className="flex-1 overflow-y-auto">
                                                    <div className="px-8 py-6">
                                                        <div className="grid gap-8 lg:grid-cols-2">
                                                            {/* CLIENTES */}
                                                            <FieldSet>
                                                                <FieldLegend className="flex items-center gap-3 text-lg font-semibold">
                                                                    <Users className="h-5 w-5 text-blue-600" />
                                                                    Financiamiento con Clientes
                                                                </FieldLegend>
                                                                <FieldDescription>Usa créditos de clientes o genera préstamos</FieldDescription>

                                                                <FieldGroup className="space-y-4 pt-4">
                                                                    <Field>
                                                                        <div ref={resolvePagoDialogContainer}>
                                                                            <Combobox
                                                                                multiple
                                                                                items={clientes}
                                                                                filteredItems={filteredClientes.filter((c) => c.id !== clienteDeCompra?.id)}
                                                                                itemToStringLabel={(c: ClienteProps) => c.nombre_cliente}
                                                                                value={data.pagos_clientes
                                                                                    .map((p) => clientes.find((c) => c.id === p.cliente_id))
                                                                                    .filter((c): c is ClienteProps => !!c)}
                                                                                onValueChange={(seleccionados) => {
                                                                                    setData(
                                                                                        'pagos_clientes',
                                                                                        seleccionados.map(
                                                                                            (c) =>
                                                                                                data.pagos_clientes.find(
                                                                                                    (p) => p.cliente_id === c.id,
                                                                                                ) ?? { cliente_id: c.id, monto: 0 },
                                                                                        ),
                                                                                    );
                                                                                }}
                                                                                onInputValueChange={setClienteSearchTerm}
                                                                            >
                                                                                <ComboboxChips>
                                                                                    {data.pagos_clientes.map((pago) => {
                                                                                        const cliente = clientes.find(
                                                                                            (c) => c.id === pago.cliente_id,
                                                                                        );
                                                                                        return cliente ? (
                                                                                            <ComboboxChip key={pago.cliente_id}>
                                                                                                {cliente.nombre_cliente}
                                                                                            </ComboboxChip>
                                                                                        ) : null;
                                                                                    })}
                                                                                    <ComboboxChipsInput placeholder="Buscar cliente..." />
                                                                                </ComboboxChips>
                                                                                <ComboboxContent container={pagoDialogContainer}>
                                                                                    <ComboboxEmpty>
                                                                                        {isSearchingClientes ? 'Buscando...' : 'Sin resultados'}
                                                                                    </ComboboxEmpty>
                                                                                    <ComboboxList>
                                                                                        {(cliente: ClienteProps) => (
                                                                                            <ComboboxItem key={cliente.id} value={cliente}>
                                                                                                <div className="flex w-full items-center justify-between gap-2">
                                                                                                    <span>{cliente.nombre_cliente}</span>
                                                                                                    <Badge
                                                                                                        variant={
                                                                                                            (cliente.deuda_pago_cliente ?? 0) > 0
                                                                                                                ? 'default'
                                                                                                                : 'secondary'
                                                                                                        }
                                                                                                        className="text-xs"
                                                                                                    >
                                                                                                        ${cliente.deuda_pago_cliente ?? 0}
                                                                                                    </Badge>
                                                                                                </div>
                                                                                            </ComboboxItem>
                                                                                        )}
                                                                                    </ComboboxList>
                                                                                    <Separator className="my-2" />
                                                                                    <div
                                                                                        className="hover:bg-accent flex cursor-pointer items-center gap-2 p-2 text-sm text-blue-600"
                                                                                        onClick={() => {
                                                                                            setCrearComoClienteDeCompra(false);
                                                                                            setIsCrearClienteDialogOpen(true);
                                                                                        }}
                                                                                    >
                                                                                        <PlusCircle className="h-4 w-4" />
                                                                                        Crear nuevo cliente
                                                                                    </div>
                                                                                </ComboboxContent>
                                                                            </Combobox>
                                                                        </div>
                                                                    </Field>

                                                                    {data.pagos_clientes.length > 0 && (
                                                                        <div className="divide-y rounded-lg border">
                                                                            {data.pagos_clientes.map((pago) => {
                                                                                const cliente = clientes.find((c) => c.id === pago.cliente_id);
                                                                                return (
                                                                                    <div
                                                                                        key={pago.cliente_id}
                                                                                        className="flex items-center gap-3 p-3"
                                                                                    >
                                                                                        <div className="bg-primary/10 text-primary flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-sm font-bold">
                                                                                            {cliente?.nombre_cliente[0]}
                                                                                        </div>
                                                                                        <div className="min-w-0 flex-1">
                                                                                            <p className="truncate text-sm font-medium">
                                                                                                {cliente?.nombre_cliente}
                                                                                            </p>
                                                                                            <p className="text-muted-foreground text-xs">
                                                                                                Crédito: ${cliente?.deuda_pago_cliente || 0}
                                                                                            </p>
                                                                                        </div>
                                                                                        <InputGroup className="w-32 shrink-0">
                                                                                            <InputGroupAddon>$</InputGroupAddon>
                                                                                            <InputGroupInput
                                                                                                inputMode="decimal"
                                                                                                placeholder="0.00"
                                                                                                value={pago.monto || ''}
                                                                                                onChange={(e) => {
                                                                                                    const monto = parseFloat(e.target.value) || 0;
                                                                                                    setData(
                                                                                                        'pagos_clientes',
                                                                                                        data.pagos_clientes.map((p) =>
                                                                                                            p.cliente_id === pago.cliente_id
                                                                                                                ? { ...p, monto }
                                                                                                                : p,
                                                                                                        ),
                                                                                                    );
                                                                                                }}
                                                                                            />
                                                                                        </InputGroup>
                                                                                        <Button
                                                                                            variant="ghost"
                                                                                            size="icon"
                                                                                            className="shrink-0 cursor-pointer"
                                                                                            onClick={() =>
                                                                                                setData(
                                                                                                    'pagos_clientes',
                                                                                                    data.pagos_clientes.filter(
                                                                                                        (p) => p.cliente_id !== pago.cliente_id,
                                                                                                    ),
                                                                                                )
                                                                                            }
                                                                                        >
                                                                                            <X className="h-4 w-4" />
                                                                                        </Button>
                                                                                    </div>
                                                                                );
                                                                            })}
                                                                        </div>
                                                                    )}
                                                                </FieldGroup>
                                                            </FieldSet>

                                                            {/* CUENTAS */}
                                                            <FieldSet>
                                                                <FieldLegend className="flex items-center gap-3 text-lg font-semibold">
                                                                    <Wallet className="h-5 w-5 text-emerald-600" />
                                                                    Pago desde Cuentas
                                                                </FieldLegend>
                                                                <FieldDescription>Selecciona las cuentas a débitar</FieldDescription>

                                                                <FieldGroup className="space-y-4 pt-4">
                                                                    <Field>
                                                                        <Combobox
                                                                            multiple
                                                                            items={cuentas}
                                                                            itemToStringLabel={(c: CuentaNegocioProps) => c.nombre_cuenta}
                                                                            value={data.pagos
                                                                                .map((p) => cuentas.find((c) => c.id === p.cuenta_id))
                                                                                .filter((c): c is CuentaNegocioProps => !!c)}
                                                                            onValueChange={(seleccionadas) => {
                                                                                setData(
                                                                                    'pagos',
                                                                                    seleccionadas.map(
                                                                                        (c) =>
                                                                                            data.pagos.find((p) => p.cuenta_id === c.id) ?? {
                                                                                                cuenta_id: c.id,
                                                                                                monto: 0,
                                                                                            },
                                                                                    ),
                                                                                );
                                                                            }}
                                                                        >
                                                                            <ComboboxChips>
                                                                                {data.pagos.map((pago) => {
                                                                                    const cuenta = cuentas.find((c) => c.id === pago.cuenta_id);
                                                                                    return cuenta ? (
                                                                                        <ComboboxChip key={pago.cuenta_id}>
                                                                                            {cuenta.nombre_cuenta}
                                                                                        </ComboboxChip>
                                                                                    ) : null;
                                                                                })}
                                                                                <ComboboxChipsInput placeholder="Buscar cuentas..." />
                                                                            </ComboboxChips>
                                                                            <ComboboxContent container={pagoDialogContainer}>
                                                                                <ComboboxEmpty>Sin resultados</ComboboxEmpty>
                                                                                <ComboboxList>
                                                                                    {(cuenta: CuentaNegocioProps) => (
                                                                                        <ComboboxItem key={cuenta.id} value={cuenta}>
                                                                                            <div className="flex w-full items-center justify-between gap-2">
                                                                                                <span>{cuenta.nombre_cuenta}</span>
                                                                                                <span className="text-muted-foreground text-xs">
                                                                                                    Saldo: ${cuenta.saldo_cuenta}
                                                                                                </span>
                                                                                            </div>
                                                                                        </ComboboxItem>
                                                                                    )}
                                                                                </ComboboxList>
                                                                            </ComboboxContent>
                                                                        </Combobox>
                                                                    </Field>

                                                                    {data.pagos.length > 0 && (
                                                                        <div className="divide-y rounded-lg border">
                                                                            {data.pagos.map((pago) => {
                                                                                const cuenta = cuentas.find((c) => c.id === pago.cuenta_id);
                                                                                return (
                                                                                    <div key={pago.cuenta_id} className="flex items-center gap-3 p-3">
                                                                                        <div className="bg-primary/10 text-primary flex h-9 w-9 shrink-0 items-center justify-center rounded-full text-sm font-bold">
                                                                                            {cuenta?.nombre_cuenta[0]}
                                                                                        </div>
                                                                                        <div className="min-w-0 flex-1">
                                                                                            <p className="truncate text-sm font-medium">
                                                                                                {cuenta?.nombre_cuenta}
                                                                                            </p>
                                                                                            <p className="text-muted-foreground text-xs">
                                                                                                Saldo: ${cuenta?.saldo_cuenta}
                                                                                            </p>
                                                                                        </div>
                                                                                        <InputGroup className="w-32 shrink-0">
                                                                                            <InputGroupAddon>$</InputGroupAddon>
                                                                                            <InputGroupInput
                                                                                                inputMode="decimal"
                                                                                                placeholder="0.00"
                                                                                                value={pago.monto || ''}
                                                                                                onChange={(e) => {
                                                                                                    const monto = parseFloat(e.target.value) || 0;
                                                                                                    setData(
                                                                                                        'pagos',
                                                                                                        data.pagos.map((p) =>
                                                                                                            p.cuenta_id === pago.cuenta_id
                                                                                                                ? { ...p, monto }
                                                                                                                : p,
                                                                                                        ),
                                                                                                    );
                                                                                                }}
                                                                                            />
                                                                                        </InputGroup>
                                                                                        <Button
                                                                                            variant="ghost"
                                                                                            size="icon"
                                                                                            className="shrink-0 cursor-pointer"
                                                                                            onClick={() =>
                                                                                                setData(
                                                                                                    'pagos',
                                                                                                    data.pagos.filter(
                                                                                                        (p) => p.cuenta_id !== pago.cuenta_id,
                                                                                                    ),
                                                                                                )
                                                                                            }
                                                                                        >
                                                                                            <X className="h-4 w-4" />
                                                                                        </Button>
                                                                                    </div>
                                                                                );
                                                                            })}
                                                                        </div>
                                                                    )}
                                                                </FieldGroup>
                                                            </FieldSet>
                                                        </div>
                                                    </div>
                                                </ScrollArea>

                                                <AlertDialogFooter className="bg-muted/50 border-t px-8 py-5">
                                                    <Button
                                                        variant="outline"
                                                        onClick={() => setData('compra', '')}
                                                        className="h-12 cursor-pointer px-6"
                                                    >
                                                        Atrás
                                                    </Button>
                                                    <Button
                                                        onClick={realizarCompra}
                                                        disabled={processing || !puedeConfirmar}
                                                        className="h-12 cursor-pointer bg-emerald-600 px-8 hover:bg-emerald-700"
                                                    >
                                                        {processing ? (
                                                            <>
                                                                Procesando <Loader2 className="ml-2 h-5 w-5 animate-spin" />
                                                            </>
                                                        ) : (
                                                            <>
                                                                Confirmar Pago <CheckCircle className="ml-2 h-5 w-5" />
                                                            </>
                                                        )}
                                                    </Button>
                                                </AlertDialogFooter>
                                            </>
                                        )}
                                    </>
                                );
                            })()}
                        </AlertDialogContent>
                    </AlertDialog>

                    {/* MODAL CREAR CLIENTE - SE ABRE CON EL BOTÓN DE ARRIBA */}
                    <Dialog open={isCrearClienteDialogOpen} onOpenChange={setIsCrearClienteDialogOpen}>
                        <CrearClienteDialogContent />
                    </Dialog>

                    {/* MODAL CREAR ALMACÉN */}
                    <Dialog open={isCrearAlmacenDialogOpen} onOpenChange={setIsCrearAlmacenDialogOpen}>
                        <CrearAlmacenDialogContent />
                    </Dialog>
                </div>

                {/* MODAL CREAR CATEGORÍA */}
                <Dialog open={isCrearCategoriaDialogOpen} onOpenChange={setIsCrearCategoriaDialogOpen}>
                    <CrearCategoriaDialogContent />
                </Dialog>

                {/* HISTORIAL DE COMPRAS */}
                {comprasRecientes.length > 0 && (
                    <Card className="mt-4 overflow-hidden border-0 pt-0 shadow-lg">
                        <CardHeader className="bg-gradient-to-r from-amber-600 to-amber-700 px-6 py-5 text-white">
                            <div className="flex items-center gap-3">
                                <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-white/20 backdrop-blur-sm">
                                    <ShoppingBasket className="h-5 w-5" />
                                </div>
                                <div>
                                    <CardTitle className="text-base font-semibold text-white">Historial de Compras Recientes</CardTitle>
                                    <CardDescription className="text-xs text-amber-100">
                                        Últimas {comprasRecientes.length} compras registradas
                                    </CardDescription>
                                </div>
                            </div>
                        </CardHeader>
                        <CardContent className="p-0">
                            <Table>
                                <TableHeader>
                                    <TableRow className="bg-gray-100 dark:bg-gray-800">
                                        <TableHead>#</TableHead>
                                        <TableHead>Fecha</TableHead>
                                        <TableHead>Asociado</TableHead>
                                        <TableHead>Tipo</TableHead>
                                        <TableHead>Estado</TableHead>
                                        <TableHead className="text-right">Total</TableHead>
                                        <TableHead className="text-center">Acciones</TableHead>
                                    </TableRow>
                                </TableHeader>
                                <TableBody>
                                    {comprasRecientes.map((compra) => (
                                        <TableRow key={compra.id}>
                                            <TableCell className="font-mono text-xs text-gray-500">#{String(compra.id).padStart(6, '0')}</TableCell>
                                            <TableCell className="text-sm">
                                                {new Date(compra.fecha_compra).toLocaleDateString('es-MX', {
                                                    day: '2-digit',
                                                    month: 'short',
                                                    year: 'numeric',
                                                })}
                                            </TableCell>
                                            <TableCell className="font-medium">{compra.proveedor ?? 'Sin registro'}</TableCell>
                                            <TableCell>
                                                <Badge
                                                    className={cn(
                                                        'text-white hover:opacity-90',
                                                        compra.tipo_compra === 'deuda_proveedor'
                                                            ? 'bg-red-500'
                                                            : compra.es_parcial
                                                              ? 'bg-amber-500'
                                                              : 'bg-emerald-500',
                                                    )}
                                                >
                                                    {compra.tipo_compra === 'deuda_proveedor' ? (
                                                        <CreditCard className="h-3 w-3" />
                                                    ) : compra.es_parcial ? (
                                                        <AlertTriangle className="h-3 w-3" />
                                                    ) : (
                                                        <DollarSign className="h-3 w-3" />
                                                    )}
                                                    {compra.tipo_compra === 'deuda_proveedor' ? 'Crédito' : compra.es_parcial ? 'Parcial' : 'Contado'}
                                                </Badge>
                                            </TableCell>
                                            <TableCell>
                                                {(() => {
                                                    const EstadoIcon = ESTADO_COMPRA_CONFIG[compra.estado].icon;
                                                    return (
                                                        <Badge className={ESTADO_COMPRA_CONFIG[compra.estado].className}>
                                                            <EstadoIcon className="h-3 w-3" />
                                                            {ESTADO_COMPRA_CONFIG[compra.estado].label}
                                                        </Badge>
                                                    );
                                                })()}
                                            </TableCell>
                                            <TableCell className="text-right font-bold text-emerald-600">
                                                ${compra.total_compra.toLocaleString('es-MX', { minimumFractionDigits: 2 })}
                                            </TableCell>
                                            <TableCell className="text-center">
                                                <Link href={`/comprar/${compra.id}`}>
                                                    <Button size="sm" variant="outline" className="gap-1">
                                                        <Info className="h-3 w-3" />
                                                        Ver detalle
                                                    </Button>
                                                </Link>
                                            </TableCell>
                                        </TableRow>
                                    ))}
                                </TableBody>
                            </Table>
                        </CardContent>
                    </Card>
                )}

                <Toaster position="top-center" />
            </div>
            <ScrollProgress />
        </AppLayout>
    );
}
