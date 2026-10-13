import AppLayout from "@/components/Layout/AppLayout";
import { useState, useEffect } from 'react';
import { router, useForm } from '@inertiajs/react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Checkbox } from '@/components/ui/checkbox';
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from '@/components/ui/table';
import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from '@/components/ui/select';
import {
    Dialog,
    DialogContent,
    DialogHeader,
    DialogTitle,
} from "@/components/ui/dialog";
import {
    Save,
    User,
    Plus,
    Wallet,
    ShoppingCart,
    ClipboardList,
    Home,
    Utensils,
    Users,
    HeartPulse,
    Car,
    BookOpen,
    CheckCircle2,
} from 'lucide-react';
import { cn, fechaHoy } from '@/lib/utils';
import { Field } from "@base-ui/react";

// ── Catálogos ─────────────────────────────────────────────────────────────────

const ocupaciones = [
    "Desempleado / Empleo temporal",
    "Obrero / Empleado",
    "Profesional",
    "Empresario",
];
const regimenes   = ["Sin vivienda", "Familiar / Prestada / Rentada", "Propia", "Más de 1 vivienda"];
const tiposViv    = ["Vecindad / Condominio / Interés social / Casa habitación", "Residencial M.", "Residencial A."];
const habitaciones = ["1 solo cuarto / 2 dormitorios", "3 dormitorios / 4 dormitorios", "Más de 4"];
const pisos       = ["Tierra / Concreto / Mosaico", "Vitropiso", "Otros"];
const muros       = ["Adobe / Tabique", "Concreto", "Otros"];
const techos      = ["Lámina cartón / Lámina asbesto", "Concreto", "Otros"];
const alimentos   = [
    { label: "Carne de res",                         key: "ali_carneRes" },
    { label: "Carne de pollo",                       key: "ali_carnePollo" },
    { label: "Carne de cerdo",                       key: "ali_carneCerdo" },
    { label: "Pescado",                              key: "ali_pescado" },
    { label: "Leche",                                key: "ali_leche" },
    { label: "Cereales",                             key: "ali_cereales" },
    { label: "Huevo",                                key: "ali_huevo" },
    { label: "Frutas",                               key: "ali_frutas" },
    { label: "Verduras",                             key: "ali_verduras" },
    { label: "Leguminosas (Frijol, lentejas, etc.)", key: "ali_legu" },
];
const frecuencias = ["Diario", "Cada 3 Días", "Semanal", "Mensual", "Ocasional"];

// ── Helpers ───────────────────────────────────────────────────────────────────

const FieldError = ({ message }) =>
    message ? <p className="mt-1 text-xs text-red-500">{message}</p> : null;

const LabelReq = ({ children, required = false, htmlFor }) => (
    <Label htmlFor={htmlFor} className="mb-1 block text-xs font-semibold uppercase tracking-[0.22em] text-slate-500">
        {children}
        {required && <span className="text-red-500 ml-1">*</span>}
    </Label>
);

const SectionTitle = ({ icon, title }) => (
    <div className="flex items-center gap-2 mb-5">
        <span className="text-teal-600">{icon}</span>
        <h2 className="text-base font-bold text-slate-800 tracking-tight">{title}</h2>
    </div>
);

const sv = (val) =>
    val !== null && val !== undefined && val !== "" ? String(val) : "";

// ── Definición de secciones ───────────────────────────────────────────────────

const SECTION_TITLES = [
    { label: "Datos del solicitante",        icon: <User size={14} /> },
    { label: "Datos del beneficiario",       icon: <User size={14} /> },
    { label: "Estructura familiar",          icon: <Users size={14} /> },
    { label: "Ingreso y egreso familiar",    icon: <Wallet size={14} /> },
    { label: "Ingresos/Egresos mensuales",   icon: <ShoppingCart size={14} /> },
    { label: "Transporte",                   icon: <Car size={14} /> },
    { label: "Salud y adicciones",           icon: <HeartPulse size={14} /> },
    { label: "Vivienda",                     icon: <Home size={14} /> },
    { label: "Alimentación y referencias",   icon: <Utensils size={14} /> },
    { label: "Diagnóstico y observaciones",  icon: <BookOpen size={14} /> },
];
const TOTAL_SECTIONS = SECTION_TITLES.length;

// ── Componente principal ──────────────────────────────────────────────────────

// ── LaboralBlock — must be defined OUTSIDE the main component so React does not
//    recreate it as a new component type on every render (which would unmount
//    all its inputs and reset values after a single keystroke).
function LaboralBlock({ prefix, title, tieneConyugeField = false, data, setData, errors }) {
    return (
        <Card>
            <CardHeader className="pb-3">
                <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">{title}</CardTitle>
            </CardHeader>
            <CardContent className="flex flex-col gap-3">
                {tieneConyugeField && (
                    <div>
                        <LabelReq>¿Cuenta con cónyuge?</LabelReq>
                        <RadioGroup className="flex flex-row gap-4 mt-1" value={data[`${prefix}_existe`]} onValueChange={v => setData(`${prefix}_existe`, v)}>
                            {["si","no"].map(v => (
                                <div key={v} className="flex items-center gap-2">
                                    <RadioGroupItem value={v} id={`${prefix}_existe_${v}`} />
                                    <Label htmlFor={`${prefix}_existe_${v}`}>{v === 'si' ? 'Sí' : 'No'}</Label>
                                </div>
                            ))}
                        </RadioGroup>
                        <FieldError message={errors[`${prefix}_existe`]} />
                    </div>
                )}
                <div>
                    <LabelReq>¿Cuenta con empleo actual?</LabelReq>
                    <RadioGroup className="flex flex-row gap-4 mt-1" value={data[`${prefix}_empleo`]} onValueChange={v => setData(`${prefix}_empleo`, v)}>
                        {["si","no"].map(v => (
                            <div key={v} className="flex items-center gap-2">
                                <RadioGroupItem value={v} id={`${prefix}_empleo_${v}`} />
                                <Label htmlFor={`${prefix}_empleo_${v}`}>{v === 'si' ? 'Sí' : 'No'}</Label>
                            </div>
                        ))}
                    </RadioGroup>
                    <FieldError message={errors[`${prefix}_empleo`]} />
                </div>
                {[
                    ["Empresa en la que labora", `${prefix}_empresa`,   "text"],
                    ["Antigüedad",               `${prefix}_antiguedad`,"text"],
                    ["Puesto que ocupa",         `${prefix}_puesto`,    "text"],
                    ["Ingreso mensual neto",      `${prefix}_mensual`,  "number"],
                    ["Otros ingresos",           `${prefix}_otros`,     "number"],
                ].map(([label, key, type]) => (
                    <div key={key}>
                        <LabelReq>{label}</LabelReq>
                        <Input
                            type={type}
                            value={data[key] ?? ''}
                            onChange={e => setData(key, e.target.value)}
                        />
                        <FieldError message={errors[key]} />
                    </div>
                ))}
                <div>
                    <LabelReq>Ocupación</LabelReq>
                    <Select value={sv(data[`${prefix}_ocupacion`])} onValueChange={v => setData(`${prefix}_ocupacion`, v)}>
                        <SelectTrigger>
                            <SelectValue placeholder="Seleccionar..." />
                        </SelectTrigger>
                        <SelectContent>
                            {ocupaciones.map(o => <SelectItem key={o} value={o}>{o}</SelectItem>)}
                        </SelectContent>
                    </Select>
                    <FieldError message={errors[`${prefix}_ocupacion`]} />
                </div>
            </CardContent>
        </Card>
    );
}

// ── Main component ────────────────────────────────────────────────────────────
export default function AdmisionesEstudioSocioeconomico({
    paciente,
    estudio = null,
    modo = 'crear',
    breadcrumbs
}) {
    const viendoExistente = modo === 'ver';
    const e = estudio ?? {};

    const [section, setSection] = useState(0);

    // Dialogs
    const [isEstFamOpen,          setEstFamOpen]          = useState(false);
    const [isOtrosMiembrosOpen,   setOtrosMiembrosOpen]   = useState(false);
    const [isTransporteOpen,      setTransporteOpen]      = useState(false);
    const [isReferenciaOpen,      setReferenciaOpen]      = useState(false);

    // Temporales para dialogs
    const [nuevoIntegrante,    setNuevoIntegrante]    = useState({ ef_nombre: '', ef_parentesco: '', ef_edad: '', ef_sexo: '', ef_estcivil: '', ef_ocupacion: '' });
    const [nuevoOtrosMiembros, setNuevoOtrosMiembros] = useState({ iefOtros_parentesco: '', iefOtros_cantidad: '' });
    const [nuevoTransporte,    setNuevoTransporte]    = useState({ transp_marca: '', transp_modelo: '', transp_propietario: '' });
    const [nuevoReferencia,    setNuevoReferencia]    = useState({ referencia_nombre: '', referencia_telefono: '', referencia_relacion: '', referencia_tiempo: '' });

    const { data, setData, post, processing, errors, wasSuccessful } = useForm({
        paciente_id:    paciente?.paciente_id ?? '',
        responsable_id: paciente?.responsable?.responsable_id ?? '',

        foto: '',
        fecha_aplicacion: '',

        // Responsable
        r_nombres: '', r_apellidos: '', r_fecha_nacimiento: '', r_lugar_nacimiento: '',
        r_sexo: '', r_escolaridad: '', r_estado_civil: '', r_ocupacion: '',
        r_telefono: '', r_celular: '', r_colonia: '', r_no_ext: '', r_no_int: '',
        r_calle: '', r_cp: '', r_municipio: '', r_estado: '', r_tarjeta: '',

        croquis_imagen: '', croquis_descripcion: '',

        // Paciente — pre-filled from paciente prop where available
        p_nombres:          paciente?.nombre     ?? '',
        p_apellidos:        paciente?.apellidos  ?? '',
        p_fecha_nacimiento: paciente?.fecha_nacimiento ? paciente.fecha_nacimiento.slice(0, 10) : '',
        p_lugar_nacimiento: '',
        p_sexo: paciente?.sexo === 'M' ? 'Masculino' : paciente?.sexo === 'F' ? 'Femenino' : '',
        p_escolaridad: '', p_estado_civil: '', p_ocupacion: '',
        p_telefono: '', p_celular: paciente?.telefono ?? '',
        p_colonia:  paciente?.colonia  ?? '',
        p_no_ext:   paciente?.no_ext   ?? '',
        p_no_int:   paciente?.no_int   ?? '',
        p_calle:    paciente?.calle    ?? '',
        p_cp:       paciente?.cp       ?? '',
        p_municipio: paciente?.municipio_nombre ?? '',
        p_estado:    paciente?.estado_nombre    ?? '',

        estructura_familiar: [],

        // Laboral solicitante
        iefS_empleo: '', iefS_empresa: '', iefS_antiguedad: '', iefS_puesto: '',
        iefS_horario: '', iefS_dependientes: '', iefS_mensual: '', iefS_otros: '', iefS_ocupacion: '',

        // Laboral cónyuge
        iefC_existe: '', iefC_empleo: '', iefC_empresa: '', iefC_antiguedad: '',
        iefC_puesto: '', iefC_horario: '', iefC_dependientes: '',
        iefC_mensual: '', iefC_otros: '', iefC_ocupacion: '',

        otrosMiembrosAportantes: [],

        ingmen_solicitante: 0, ingmen_conyuge: 0, ingmen_otros: 0, ingmen_total: 0,
        egmen_alimentacion: '', egmen_renta: '', egmen_luz: '', egmen_agua: '',
        egmen_combustible: '', egmen_transporte: '', egmen_educacion: '',
        egmen_telefono: '', egmen_medicos: '', egmen_esparcimiento: '', egmen_otros: '',
        egmen_total: 0,
        superavitmensual: 0,

        transportes: [],

        imss: false, issste: false, seguroPopular: false, consultaParticular: false,
        sya_montoConsultas: '', sya_otros: '', sya_miembrosAsistenciaMedica: '',
        alcoholismo: '', alcoholismo_desc: '',
        drogadiccion: '', drogadiccion_desc: '',
        tca: '', tca_desc: '',
        ludopatia: '', ludopatia_desc: '',
        adicciones_otros: '', adicciones_otros_desc: '',
        adicciones_relacionFamDesc: '',

        viv_regimen: '', viv_tipo: '', viv_espacio: '',
        viv_sala: false, viv_comedor: false, viv_cocina: false, viv_jardin: false,
        viv_banos: '', viv_otros: '',
        viv_piso: '', viv_muros: '', viv_techo: '',

        ali_carneRes: 'Ocasional', ali_carnePollo: 'Ocasional', ali_carneCerdo: 'Ocasional',
        ali_pescado: 'Ocasional', ali_leche: 'Ocasional', ali_cereales: 'Ocasional',
        ali_huevo: 'Ocasional', ali_frutas: 'Ocasional', ali_verduras: 'Ocasional',
        ali_legu: 'Ocasional',

        referenciasPersonales: [],

        diagnostico_desc: '',
        obs_trabajadorSocial: '',
        obs_visitaDom: '',
    });

    // Auto-cálculo ingresos/egresos
    // NOTE: We compute derived values and write them only if they changed to avoid
    // clobbering in-flight input state. We do NOT spread prev — we call setData per key.
    useEffect(() => {
        const solicitante = (parseFloat(data.iefS_mensual) || 0) + (parseFloat(data.iefS_otros) || 0);
        const conyuge     = (parseFloat(data.iefC_mensual) || 0) + (parseFloat(data.iefC_otros) || 0);
        const otros       = data.otrosMiembrosAportantes.reduce((s, r) => s + (parseFloat(r.iefOtros_cantidad) || 0), 0);
        const ing_total   = solicitante + conyuge + otros;
        const egFields    = ['egmen_alimentacion','egmen_renta','egmen_luz','egmen_agua','egmen_combustible','egmen_transporte','egmen_educacion','egmen_telefono','egmen_medicos','egmen_esparcimiento','egmen_otros'];
        const eg_total    = egFields.reduce((s, k) => s + (parseFloat(data[k]) || 0), 0);

        // Only update derived read-only totals — never overwrite user-typed fields
        if (data.ingmen_solicitante !== solicitante) setData('ingmen_solicitante', solicitante);
        if (data.ingmen_conyuge     !== conyuge)     setData('ingmen_conyuge',     conyuge);
        if (data.ingmen_otros       !== otros)       setData('ingmen_otros',       otros);
        if (data.ingmen_total       !== ing_total)   setData('ingmen_total',       ing_total);
        if (data.egmen_total        !== eg_total)    setData('egmen_total',        eg_total);
        if (data.superavitmensual   !== ing_total - eg_total) setData('superavitmensual', ing_total - eg_total);
    }, [
        data.iefS_mensual, data.iefS_otros, data.iefC_mensual, data.iefC_otros,
        data.otrosMiembrosAportantes,
        data.egmen_alimentacion, data.egmen_renta, data.egmen_luz, data.egmen_agua,
        data.egmen_combustible, data.egmen_transporte, data.egmen_educacion,
        data.egmen_telefono, data.egmen_medicos, data.egmen_esparcimiento, data.egmen_otros,
    ]);

    // Poblar desde estudio existente
    useEffect(() => {
        if (!viendoExistente || !estudio) return;
        setData(prev => ({
            ...prev, ...e,
            fecha_aplicacion:        e.fecha_aplicacion ? String(e.fecha_aplicacion).slice(0, 10) : '',
            estructura_familiar:     Array.isArray(e.estructura_familiar)     ? e.estructura_familiar     : [],
            otrosMiembrosAportantes: Array.isArray(e.otrosMiembrosAportantes) ? e.otrosMiembrosAportantes : [],
            transportes:             Array.isArray(e.transportes)             ? e.transportes             : [],
            referenciasPersonales:   Array.isArray(e.referenciasPersonales)   ? e.referenciasPersonales   : [],
        }));
    }, []);

    // Paciente fields are pre-filled directly in useForm initial values above.

    const calcularEdad = (fecha) => {
        if (!fecha) return '';
        const diff = Date.now() - new Date(fecha).getTime();
        return Math.abs(new Date(diff).getUTCFullYear() - 1970);
    };
    const edadResponsable = calcularEdad(data.r_fecha_nacimiento);
    const edadPaciente    = calcularEdad(data.p_fecha_nacimiento);

    // Dialog handlers
    const estructuraFamAddRow = () => {
        if (!nuevoIntegrante.ef_nombre) return;
        setData('estructura_familiar', [...data.estructura_familiar, { ...nuevoIntegrante, id: Date.now() }]);
        setNuevoIntegrante({ ef_nombre: '', ef_parentesco: '', ef_edad: '', ef_sexo: '', ef_estcivil: '', ef_ocupacion: '' });
        setEstFamOpen(false);
    };
    const otrosMiembrosAddRow = () => {
        if (!nuevoOtrosMiembros.iefOtros_parentesco || !nuevoOtrosMiembros.iefOtros_cantidad) return;
        setData('otrosMiembrosAportantes', [...data.otrosMiembrosAportantes, { ...nuevoOtrosMiembros, id: Date.now() }]);
        setNuevoOtrosMiembros({ iefOtros_parentesco: '', iefOtros_cantidad: '' });
        setOtrosMiembrosOpen(false);
    };
    const transporteAddRow = () => {
        if (!nuevoTransporte.transp_marca || !nuevoTransporte.transp_modelo || !nuevoTransporte.transp_propietario) return;
        setData('transportes', [...data.transportes, { ...nuevoTransporte, id: Date.now() }]);
        setNuevoTransporte({ transp_marca: '', transp_modelo: '', transp_propietario: '' });
        setTransporteOpen(false);
    };
    const referenciaAddRow = () => {
        if (!nuevoReferencia.referencia_nombre || !nuevoReferencia.referencia_relacion) return;
        setData('referenciasPersonales', [...data.referenciasPersonales, { ...nuevoReferencia, id: Date.now() }]);
        setNuevoReferencia({ referencia_nombre: '', referencia_telefono: '', referencia_relacion: '', referencia_tiempo: '' });
        setReferenciaOpen(false);
    };

    const handleSubmit = (ev) => {
        ev.preventDefault();
        post("/admisiones/socioeconomico", {
            preserveScroll: true,
            onError: () => window.scrollTo({ top: 0, behavior: 'smooth' }),
        });
    };

    const progressPct = Math.round(((section + 1) / TOTAL_SECTIONS) * 100);

    // ── Secciones ────────────────────────────────────────────────────────────

    const S1 = (
        <Card>

            <CardContent className="flex flex-col gap-4">
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                        <LabelReq required>Nombre(s)</LabelReq>
                        <Input value={data.r_nombres} onChange={e => setData('r_nombres', e.target.value)} />
                        <FieldError message={errors.r_nombres} />
                    </div>
                    <div>
                        <LabelReq required>Apellidos</LabelReq>
                        <Input value={data.r_apellidos} onChange={e => setData('r_apellidos', e.target.value)} />
                        <FieldError message={errors.r_apellidos} />
                    </div>
                    <div>
                        <LabelReq>Fecha de nacimiento</LabelReq>
                        <Input type="date" value={data.r_fecha_nacimiento} onChange={e => setData('r_fecha_nacimiento', e.target.value)} />
                        <FieldError message={errors.r_fecha_nacimiento} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                        <LabelReq>Lugar de nacimiento</LabelReq>
                        <Input value={data.r_lugar_nacimiento} onChange={e => setData('r_lugar_nacimiento', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>Edad</LabelReq>
                        <Input readOnly value={edadResponsable !== '' ? `${edadResponsable} años` : ''} />
                    </div>
                    <div>
                        <LabelReq>Sexo</LabelReq>
                        <Select value={sv(data.r_sexo)} onValueChange={v => setData('r_sexo', v)}>
                            <SelectTrigger><SelectValue placeholder="Seleccionar..." /></SelectTrigger>
                            <SelectContent>
                                {["Masculino","Femenino","Otro"].map(o => <SelectItem key={o} value={o}>{o}</SelectItem>)}
                            </SelectContent>
                        </Select>
                        <FieldError message={errors.r_sexo} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                        <LabelReq required>Escolaridad</LabelReq>
                        <Input value={data.r_escolaridad} onChange={e => setData('r_escolaridad', e.target.value)} />
                        <FieldError message={errors.r_escolaridad} />
                    </div>
                    <div>
                        <LabelReq required>Estado civil</LabelReq>
                        <Input value={data.r_estado_civil} onChange={e => setData('r_estado_civil', e.target.value)} />
                        <FieldError message={errors.r_estado_civil} />
                    </div>
                    <div>
                        <LabelReq required>Ocupación</LabelReq>
                        <Input value={data.r_ocupacion} onChange={e => setData('r_ocupacion', e.target.value)} />
                        <FieldError message={errors.r_ocupacion} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                    <div>
                        <LabelReq>No. telefónico</LabelReq>
                        <Input type="tel" value={data.r_telefono} onChange={e => setData('r_telefono', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>Teléfono celular</LabelReq>
                        <Input type="tel" value={data.r_celular} onChange={e => setData('r_celular', e.target.value)} />
                    </div>
                </div>
                <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
                    <div className="col-span-2">
                        <LabelReq>Colonia</LabelReq>
                        <Input value={data.r_colonia} onChange={e => setData('r_colonia', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>No. Ext</LabelReq>
                        <Input value={data.r_no_ext} onChange={e => setData('r_no_ext', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>No. Int</LabelReq>
                        <Input value={data.r_no_int} onChange={e => setData('r_no_int', e.target.value)} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                        <LabelReq>Calle</LabelReq>
                        <Input value={data.r_calle} onChange={e => setData('r_calle', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>C.P.</LabelReq>
                        <Input value={data.r_cp} onChange={e => setData('r_cp', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>Tarjeta crédito/débito</LabelReq>
                        <Input value={data.r_tarjeta} onChange={e => setData('r_tarjeta', e.target.value)} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                    <div>
                        <LabelReq required>Municipio</LabelReq>
                        <Input value={data.r_municipio} onChange={e => setData('r_municipio', e.target.value)} />
                        <FieldError message={errors.r_municipio} />
                    </div>
                    <div>
                        <LabelReq required>Estado</LabelReq>
                        <Input value={data.r_estado} onChange={e => setData('r_estado', e.target.value)} />
                        <FieldError message={errors.r_estado} />
                    </div>
                </div>
                <div>
                    <LabelReq>Fecha de aplicación</LabelReq>
                    <Input type="date" value={data.fecha_aplicacion} onChange={e => setData('fecha_aplicacion', e.target.value)} />
                    <FieldError message={errors.fecha_aplicacion} />
                </div>
            </CardContent>
        </Card>
    );

    const S2 = (
        <Card>
            <CardContent className="flex flex-col gap-4">
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                        <LabelReq required>Nombre(s)</LabelReq>
                        <Input value={data.p_nombres} onChange={e => setData('p_nombres', e.target.value)} />
                        <FieldError message={errors.p_nombres} />
                    </div>
                    <div>
                        <LabelReq required>Apellidos</LabelReq>
                        <Input value={data.p_apellidos} onChange={e => setData('p_apellidos', e.target.value)} />
                        <FieldError message={errors.p_apellidos} />
                    </div>
                    <div>
                        <LabelReq>Fecha de nacimiento</LabelReq>
                        <Input type="date" value={data.p_fecha_nacimiento} onChange={e => setData('p_fecha_nacimiento', e.target.value)} />
                        <FieldError message={errors.p_fecha_nacimiento} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                        <LabelReq>Lugar de nacimiento</LabelReq>
                        <Input value={data.p_lugar_nacimiento} onChange={e => setData('p_lugar_nacimiento', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>Edad</LabelReq>
                        <Input readOnly value={edadPaciente !== '' ? `${edadPaciente} años` : ''} />
                    </div>
                    <div>
                        <LabelReq>Sexo</LabelReq>
                        <Select value={sv(data.p_sexo)} onValueChange={v => setData('p_sexo', v)}>
                            <SelectTrigger><SelectValue placeholder="Seleccionar..." /></SelectTrigger>
                            <SelectContent>
                                {["Masculino","Femenino","Otro"].map(o => <SelectItem key={o} value={o}>{o}</SelectItem>)}
                            </SelectContent>
                        </Select>
                        <FieldError message={errors.p_sexo} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                    <div>
                        <LabelReq required>Escolaridad</LabelReq>
                        <Input value={data.p_escolaridad} onChange={e => setData('p_escolaridad', e.target.value)} />
                        <FieldError message={errors.p_escolaridad} />
                    </div>
                    <div>
                        <LabelReq required>Estado civil</LabelReq>
                        <Input value={data.p_estado_civil} onChange={e => setData('p_estado_civil', e.target.value)} />
                        <FieldError message={errors.p_estado_civil} />
                    </div>
                    <div>
                        <LabelReq required>Ocupación</LabelReq>
                        <Input value={data.p_ocupacion} onChange={e => setData('p_ocupacion', e.target.value)} />
                        <FieldError message={errors.p_ocupacion} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                    <div>
                        <LabelReq>No. telefónico</LabelReq>
                        <Input type="tel" value={data.p_telefono} onChange={e => setData('p_telefono', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>Teléfono celular</LabelReq>
                        <Input type="tel" value={data.p_celular} onChange={e => setData('p_celular', e.target.value)} />
                    </div>
                </div>
                <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
                    <div className="col-span-2">
                        <LabelReq>Colonia</LabelReq>
                        <Input value={data.p_colonia} onChange={e => setData('p_colonia', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>No. Ext</LabelReq>
                        <Input value={data.p_no_ext} onChange={e => setData('p_no_ext', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>No. Int</LabelReq>
                        <Input value={data.p_no_int} onChange={e => setData('p_no_int', e.target.value)} />
                    </div>
                </div>
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-4">
                    <div className="col-span-2">
                        <LabelReq>Calle</LabelReq>
                        <Input value={data.p_calle} onChange={e => setData('p_calle', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq>C.P.</LabelReq>
                        <Input value={data.p_cp} onChange={e => setData('p_cp', e.target.value)} />
                    </div>
                    <div>
                        <LabelReq required>Municipio</LabelReq>
                        <Input value={data.p_municipio} onChange={e => setData('p_municipio', e.target.value)} />
                        <FieldError message={errors.p_municipio} />
                    </div>
                </div>
                <div>
                    <LabelReq required>Estado</LabelReq>
                    <Input value={data.p_estado} onChange={e => setData('p_estado', e.target.value)} />
                    <FieldError message={errors.p_estado} />
                </div>
            </CardContent>
        </Card>
    );

    const S3 = (
        <Card>
            <CardContent>
                <div className="flex items-center justify-between mb-4">
                    <p className="text-sm text-slate-500">Registra a todos los integrantes del hogar.</p>
                    <Button type="button" size="sm" onClick={() => setEstFamOpen(true)}>
                        <Plus size={14} className="mr-1" /> Agregar integrante
                    </Button>
                </div>
                <div className="overflow-x-auto">
                    <Table>
                        <TableHeader>
                            <TableRow className="bg-slate-50">
                                {["Nombre","Parentesco","Edad","Sexo","Estado Civil","Ocupación"].map(h => (
                                    <TableHead key={h} className="text-xs uppercase tracking-wider text-slate-500">{h}</TableHead>
                                ))}
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {data.estructura_familiar.length === 0 && (
                                <TableRow>
                                    <TableCell colSpan={6} className="text-center text-sm text-slate-400 py-6">
                                        Sin integrantes registrados
                                    </TableCell>
                                </TableRow>
                            )}
                            {data.estructura_familiar.map(u => (
                                <TableRow key={u.id}>
                                    <TableCell>{u.ef_nombre}</TableCell>
                                    <TableCell>{u.ef_parentesco}</TableCell>
                                    <TableCell>{u.ef_edad}</TableCell>
                                    <TableCell>{u.ef_sexo}</TableCell>
                                    <TableCell>{u.ef_estcivil}</TableCell>
                                    <TableCell>{u.ef_ocupacion}</TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                </div>

                <Dialog open={isEstFamOpen} onOpenChange={setEstFamOpen}>
                    <DialogContent>
                        <DialogHeader><DialogTitle>Nuevo Integrante Familiar</DialogTitle></DialogHeader>
                        <div className="grid gap-3 py-2">
                            {[
                                ["Nombre",       "ef_nombre",     "text"],
                                ["Parentesco",   "ef_parentesco", "text"],
                                ["Edad",         "ef_edad",       "number"],
                                ["Sexo",         "ef_sexo",       "text"],
                                ["Estado Civil", "ef_estcivil",   "text"],
                                ["Ocupación",    "ef_ocupacion",  "text"],
                            ].map(([label, key, type]) => (
                                <div key={key}>
                                    <Label className="text-xs uppercase tracking-wider text-slate-500">{label}</Label>
                                    <Input type={type} value={nuevoIntegrante[key]}
                                        onChange={e => setNuevoIntegrante({ ...nuevoIntegrante, [key]: e.target.value })} />
                                </div>
                            ))}
                        </div>
                        <Button type="button" onClick={estructuraFamAddRow}>Confirmar y Agregar</Button>
                    </DialogContent>
                </Dialog>
            </CardContent>
        </Card>
    );

    const S4 = (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
            <LaboralBlock prefix="iefS" title="Detalles laborales — Solicitante" data={data} setData={setData} errors={errors} />
            <LaboralBlock prefix="iefC" title="Detalles laborales — Cónyuge" tieneConyugeField data={data} setData={setData} errors={errors} />
            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">
                        Otros miembros aportantes
                    </CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="overflow-x-auto mb-3">
                        <Table>
                            <TableHeader>
                                <TableRow className="bg-slate-50">
                                    <TableHead className="text-xs uppercase tracking-wider text-slate-500">Parentesco</TableHead>
                                    <TableHead className="text-xs uppercase tracking-wider text-slate-500">Cantidad ($)</TableHead>
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {data.otrosMiembrosAportantes.length === 0 && (
                                    <TableRow>
                                        <TableCell colSpan={2} className="text-center text-xs text-slate-400 py-4">Sin registros</TableCell>
                                    </TableRow>
                                )}
                                {data.otrosMiembrosAportantes.map(u => (
                                    <TableRow key={u.id}>
                                        <TableCell>{u.iefOtros_parentesco}</TableCell>
                                        <TableCell>{u.iefOtros_cantidad}</TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    </div>
                    <Button type="button" size="sm" onClick={() => setOtrosMiembrosOpen(true)}>
                        <Plus size={14} className="mr-1" /> Agregar
                    </Button>
                    <Dialog open={isOtrosMiembrosOpen} onOpenChange={setOtrosMiembrosOpen}>
                        <DialogContent>
                            <DialogHeader><DialogTitle>Nuevo miembro aportante</DialogTitle></DialogHeader>
                            <div className="grid gap-3 py-2">
                                {[["Parentesco","iefOtros_parentesco","text"],["Cantidad","iefOtros_cantidad","number"]].map(([label,key,type]) => (
                                    <div key={key}>
                                        <Label className="text-xs uppercase tracking-wider text-slate-500">{label}</Label>
                                        <Input type={type} value={nuevoOtrosMiembros[key]}
                                            onChange={e => setNuevoOtrosMiembros({ ...nuevoOtrosMiembros, [key]: e.target.value })} />
                                    </div>
                                ))}
                            </div>
                            <Button type="button" onClick={otrosMiembrosAddRow}>Confirmar y Agregar</Button>
                        </DialogContent>
                    </Dialog>
                </CardContent>
            </Card>
        </div>
    );

    const egresosFields = [
        ["Alimentación",  "egmen_alimentacion"],
        ["Renta o predio","egmen_renta"],
        ["Luz",           "egmen_luz"],
        ["Agua",          "egmen_agua"],
        ["Combustible",   "egmen_combustible"],
        ["Transporte",    "egmen_transporte"],
        ["Educación",     "egmen_educacion"],
        ["Teléfono",      "egmen_telefono"],
        ["Gastos médicos","egmen_medicos"],
        ["Esparcimiento", "egmen_esparcimiento"],
        ["Otros",         "egmen_otros"],
    ];

    const S5 = (
        <div className="grid grid-cols-1 gap-4 md:grid-cols-3 items-start">
            {/* Ingresos */}
            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-teal-700 uppercase tracking-wide flex items-center gap-2">
                        <Wallet size={14} /> Ingresos mensuales
                    </CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                    {[
                        ["Solicitante", data.ingmen_solicitante],
                        ["Cónyuge",     data.ingmen_conyuge],
                        ["Otros",       data.ingmen_otros],
                    ].map(([label, val]) => (
                        <div key={label} className="flex justify-between items-center">
                            <span className="text-xs font-semibold uppercase tracking-wider text-slate-500">{label}</span>
                            <span className="w-24 h-8 flex items-center justify-end rounded-md bg-slate-100 px-3 font-bold text-slate-800 text-sm">{val}</span>
                        </div>
                    ))}
                    <hr className="my-2 border-teal-100" />
                    <div className="flex justify-between items-center text-teal-700 font-bold">
                        <span className="text-xs uppercase tracking-wide">Total ingresos</span>
                        <span>$ {parseFloat(data.ingmen_total || 0).toFixed(2)}</span>
                    </div>
                </CardContent>
            </Card>

            {/* Egresos */}
            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-red-700 uppercase tracking-wide flex items-center gap-2">
                        <ShoppingCart size={14} /> Egresos mensuales
                    </CardTitle>
                </CardHeader>
                <CardContent className="space-y-2">
                    {egresosFields.map(([label, key]) => (
                        <div key={key} className="flex flex-col gap-0.5">
                            <div className="flex justify-between items-center gap-2">
                                <span className="text-xs font-semibold uppercase tracking-wider text-slate-500">{label}</span>
                                <Input
                                    type="number"
                                    value={data[key]}
                                    onChange={e => setData(key, e.target.value)}
                                    className={cn("w-24 h-7 text-right text-sm", errors[key] && "border-red-300")}
                                />
                            </div>
                            <FieldError message={errors[key]} />
                        </div>
                    ))}
                    <hr className="my-2 border-red-100" />
                    <div className="flex justify-between items-center text-red-700 font-bold">
                        <span className="text-xs uppercase tracking-wide">Total egresos</span>
                        <span>$ {parseFloat(data.egmen_total || 0).toFixed(2)}</span>
                    </div>
                </CardContent>
            </Card>

            {/* Superávit */}
            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">
                        {parseFloat(data.superavitmensual || 0) < 0 ? 'Déficit mensual' : parseFloat(data.superavitmensual || 0) === 0 ? 'Neutral' : 'Superávit mensual'}
                    </CardTitle>
                </CardHeader>
                <CardContent>
                    <p className={cn("text-3xl font-bold", data.superavitmensual >= 0 ? "text-teal-700" : "text-red-600")}>
                        $ {parseFloat(data.superavitmensual || 0).toFixed(2)}
                    </p>
                </CardContent>
            </Card>
        </div>
    );

    const S6 = (
        <Card>
            <CardContent>
                <div className="flex items-center justify-between mb-4">
                    <p className="text-sm text-slate-500">Vehículos con que cuenta el hogar.</p>
                    <Button type="button" size="sm" onClick={() => setTransporteOpen(true)}>
                        <Plus size={14} className="mr-1" /> Agregar
                    </Button>
                </div>
                <div className="overflow-x-auto">
                    <Table>
                        <TableHeader>
                            <TableRow className="bg-slate-50">
                                {["Marca","Modelo","Propietario"].map(h => (
                                    <TableHead key={h} className="text-xs uppercase tracking-wider text-slate-500">{h}</TableHead>
                                ))}
                            </TableRow>
                        </TableHeader>
                        <TableBody>
                            {data.transportes.length === 0 && (
                                <TableRow>
                                    <TableCell colSpan={3} className="text-center text-sm text-slate-400 py-6">Sin vehículos registrados</TableCell>
                                </TableRow>
                            )}
                            {data.transportes.map(t => (
                                <TableRow key={t.id}>
                                    <TableCell>{t.transp_marca}</TableCell>
                                    <TableCell>{t.transp_modelo}</TableCell>
                                    <TableCell>{t.transp_propietario}</TableCell>
                                </TableRow>
                            ))}
                        </TableBody>
                    </Table>
                </div>
                <Dialog open={isTransporteOpen} onOpenChange={setTransporteOpen}>
                    <DialogContent>
                        <DialogHeader><DialogTitle>Nuevo Transporte</DialogTitle></DialogHeader>
                        <div className="grid gap-3 py-2">
                            {[["Marca","transp_marca"],["Modelo","transp_modelo"],["Propietario","transp_propietario"]].map(([label,key]) => (
                                <div key={key}>
                                    <Label className="text-xs uppercase tracking-wider text-slate-500">{label}</Label>
                                    <Input value={nuevoTransporte[key]}
                                        onChange={e => setNuevoTransporte({ ...nuevoTransporte, [key]: e.target.value })} />
                                </div>
                            ))}
                        </div>
                        <Button type="button" onClick={transporteAddRow}>Confirmar y Agregar</Button>
                    </DialogContent>
                </Dialog>
            </CardContent>
        </Card>
    );

    const adiccionesList = [
        ["Alcoholismo",  "alcoholismo",       "alcoholismo_desc"],
        ["Drogadicción", "drogadiccion",      "drogadiccion_desc"],
        ["TCA",          "tca",               "tca_desc"],
        ["Ludopatía",    "ludopatia",         "ludopatia_desc"],
        ["Otros",        "adicciones_otros",  "adicciones_otros_desc"],
    ];

    const S7 = (
        <div className="flex flex-col gap-4">
            <Card>
                <CardContent>
                    <p className="text-sm font-semibold text-slate-700 mb-3">Asistencia Médica</p>
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-5">
                        {[
                            ["IMSS",               "imss"],
                            ["ISSSTE",             "issste"],
                            ["Seguro popular",     "seguroPopular"],
                            ["Consulta particular","consultaParticular"],
                        ].map(([label, key]) => (
                            <label key={key} className="flex items-center gap-3 bg-slate-50 p-3 rounded-lg cursor-pointer hover:bg-slate-100">
                                <Checkbox checked={data[key]} onCheckedChange={v => setData(key, v)} />
                                <span className="text-sm font-medium text-slate-800">{label}</span>
                            </label>
                        ))}
                    </div>
                    <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                        <div>
                            <LabelReq>Monto en consultas</LabelReq>
                            <Input value={data.sya_montoConsultas} onChange={e => setData('sya_montoConsultas', e.target.value)} />
                            <FieldError message={errors.sya_montoConsultas} />
                        </div>
                        <div>
                            <LabelReq>Otros</LabelReq>
                            <Input value={data.sya_otros} onChange={e => setData('sya_otros', e.target.value)} />
                            <FieldError message={errors.sya_otros} />
                        </div>
                        <div>
                            <LabelReq>Miembros con asistencia médica</LabelReq>
                            <Input value={data.sya_miembrosAsistenciaMedica} onChange={e => setData('sya_miembrosAsistenciaMedica', e.target.value)} />
                            <FieldError message={errors.sya_miembrosAsistenciaMedica} />
                        </div>
                    </div>
                </CardContent>
            </Card>

            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">Adicciones</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                    {adiccionesList.map(([label, keyRadio, keyDesc]) => (
                        <div key={keyRadio} className="flex flex-col gap-1">
                            <div className="flex items-center gap-4">
                                <span className="text-xs font-semibold uppercase tracking-wider text-slate-500 w-24">{label}</span>
                                <RadioGroup className="flex flex-row gap-4" value={data[keyRadio]} onValueChange={v => setData(keyRadio, v)}>
                                    {["si","no"].map(v => (
                                        <div key={v} className="flex gap-2">
                                            <RadioGroupItem value={v} id={`${keyRadio}_${v}`} />
                                            <Label htmlFor={`${keyRadio}_${v}`}>{v === 'si' ? 'Sí' : 'No'}</Label>
                                        </div>
                                    ))}
                                </RadioGroup>
                                <FieldError message={errors[keyRadio]} />
                            </div>
                            <Input
                                placeholder="Descripción..."
                                value={data[keyDesc]}
                                onChange={e => setData(keyDesc, e.target.value)}
                            />
                        </div>
                    ))}
                    <div>
                        <LabelReq>Relación familiar (describir)</LabelReq>
                        <Textarea rows={4} value={data.adicciones_relacionFamDesc} onChange={e => setData('adicciones_relacionFamDesc', e.target.value)} />
                        <FieldError message={errors.adicciones_relacionFamDesc} />
                    </div>
                </CardContent>
            </Card>
        </div>
    );

    const vivSinPropiedad = ['Sin vivienda', 'Familiar / Prestada / Rentada'].includes(data.viv_regimen);

    const S8 = (
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-3 items-start">
            {/* Régimen */}
            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">Vivienda — Régimen</CardTitle>
                    <p className="text-xs text-slate-500">Sección 9 de {TOTAL_SECTIONS}</p>
                </CardHeader>
                <CardContent className="space-y-4">
                    {[
                        ["Régimen",  "viv_regimen",  regimenes, false],
                        ["Tipo",     "viv_tipo",     tiposViv,  vivSinPropiedad],
                        ["Espacio",  "viv_espacio",  habitaciones, vivSinPropiedad],
                    ].map(([label, key, opts, isDisabled]) => (
                        <div key={key} className={isDisabled ? 'opacity-40 pointer-events-none' : ''}>
                            <LabelReq>{label}</LabelReq>
                            <Select value={sv(data[key])} onValueChange={v => setData(key, v)} disabled={isDisabled}>
                                <SelectTrigger className="h-auto min-h-9 whitespace-normal [&>span]:whitespace-normal [&>span]:text-left"><SelectValue placeholder="Seleccionar..." /></SelectTrigger>
                                <SelectContent>
                                    {opts.map(o => <SelectItem key={o} value={o} className="whitespace-normal">{o}</SelectItem>)}
                                </SelectContent>
                            </Select>
                            <FieldError message={errors[key]} />
                        </div>
                    ))}
                </CardContent>
            </Card>

            {/* Características */}
            <Card className={vivSinPropiedad ? 'opacity-40 pointer-events-none' : ''}>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">Características</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="grid grid-cols-2 gap-3 mb-4">
                        {[["Sala","viv_sala"],["Comedor","viv_comedor"],["Cocina","viv_cocina"],["Jardín","viv_jardin"]].map(([label, key]) => (
                            <label key={key} className="flex items-center gap-3 bg-slate-50 p-3 rounded-lg cursor-pointer hover:bg-slate-100">
                                <Checkbox checked={data[key]} onCheckedChange={v => setData(key, v)} disabled={vivSinPropiedad} />
                                <span className="text-sm font-medium">{label}</span>
                            </label>
                        ))}
                    </div>
                    <div className="space-y-3">
                        <div>
                            <LabelReq>Baños</LabelReq>
                            <Input disabled={vivSinPropiedad} value={data.viv_banos} onChange={e => setData('viv_banos', e.target.value)} />
                        </div>
                        <div>
                            <LabelReq>Otros</LabelReq>
                            <Input disabled={vivSinPropiedad} value={data.viv_otros} onChange={e => setData('viv_otros', e.target.value)} />
                        </div>
                    </div>
                </CardContent>
            </Card>

            {/* Material */}
            <Card className={vivSinPropiedad ? 'opacity-40 pointer-events-none' : ''}>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">Material de construcción</CardTitle>
                </CardHeader>
                <CardContent className="space-y-4">
                    {[
                        ["Piso",  "viv_piso",  pisos],
                        ["Muros", "viv_muros", muros],
                        ["Techo", "viv_techo", techos],
                    ].map(([label, key, opts]) => (
                        <div key={key}>
                            <LabelReq>{label}</LabelReq>
                            <Select value={sv(data[key])} onValueChange={v => setData(key, v)} disabled={vivSinPropiedad}>
                                <SelectTrigger className="h-auto min-h-9 whitespace-normal [&>span]:whitespace-normal [&>span]:text-left"><SelectValue placeholder="Seleccionar..." /></SelectTrigger>
                                <SelectContent>
                                    {opts.map(o => <SelectItem key={o} value={o} className="whitespace-normal">{o}</SelectItem>)}
                                </SelectContent>
                            </Select>
                            <FieldError message={errors[key]} />
                        </div>
                    ))}
                </CardContent>
            </Card>
        </div>
    );

    const S9 = (
        <div className="flex flex-col gap-4">
            {/* Alimentación */}
            <Card>
                <CardContent>
                    <div className="overflow-x-auto">
                        <table className="w-full text-left border-collapse">
                            <thead>
                                <tr className="bg-slate-50">
                                    <th className="p-3 text-xs font-bold uppercase tracking-wider text-slate-500">Tipo de alimento</th>
                                    {frecuencias.map(f => (
                                        <th key={f} className="p-3 text-xs font-bold uppercase tracking-wider text-slate-500 text-center">{f}</th>
                                    ))}
                                </tr>
                            </thead>
                            <tbody className="divide-y divide-slate-100">
                                {alimentos.map(({ label, key }) => (
                                    <tr key={key} className="hover:bg-slate-50">
                                        <td className="p-3 text-sm font-medium text-slate-800">{label}</td>
                                        {frecuencias.map(f => (
                                            <td key={f} className="p-3 text-center">
                                                <input
                                                    type="radio"
                                                    name={key}
                                                    value={f}
                                                    checked={data[key] === f}
                                                    onChange={() => setData(key, f)}
                                                    className="accent-teal-600 h-4 w-4 cursor-pointer"
                                                />
                                            </td>
                                        ))}
                                    </tr>
                                ))}
                            </tbody>
                        </table>
                    </div>
                </CardContent>
            </Card>

            {/* Referencias personales */}
            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-base font-bold tracking-wide text-teal-700 uppercase">Referencias Personales</CardTitle>
                </CardHeader>
                <CardContent>
                    <div className="flex items-center justify-between mb-4">
                        <p className="text-sm text-slate-500">Mínimo 2 referencias que no sean familiares directos.</p>
                        <Button type="button" size="sm" onClick={() => setReferenciaOpen(true)}>
                            <Plus size={14} className="mr-1" /> Agregar
                        </Button>
                    </div>
                    <div className="overflow-x-auto">
                        <Table>
                            <TableHeader>
                                <TableRow className="bg-slate-50">
                                    {["Nombre","Teléfono","Relación","Tiempo de conocerse"].map(h => (
                                        <TableHead key={h} className="text-xs uppercase tracking-wider text-slate-500">{h}</TableHead>
                                    ))}
                                </TableRow>
                            </TableHeader>
                            <TableBody>
                                {data.referenciasPersonales.length === 0 && (
                                    <TableRow>
                                        <TableCell colSpan={4} className="text-center text-sm text-slate-400 py-6">Sin referencias registradas</TableCell>
                                    </TableRow>
                                )}
                                {data.referenciasPersonales.map(r => (
                                    <TableRow key={r.id}>
                                        <TableCell>{r.referencia_nombre}</TableCell>
                                        <TableCell>{r.referencia_telefono}</TableCell>
                                        <TableCell>{r.referencia_relacion}</TableCell>
                                        <TableCell>{r.referencia_tiempo} años</TableCell>
                                    </TableRow>
                                ))}
                            </TableBody>
                        </Table>
                    </div>
                    <Dialog open={isReferenciaOpen} onOpenChange={setReferenciaOpen}>
                        <DialogContent>
                            <DialogHeader><DialogTitle>Nueva Referencia Personal</DialogTitle></DialogHeader>
                            <div className="grid gap-3 py-2">
                                {[
                                    ["Nombre",              "referencia_nombre",    "text"],
                                    ["Teléfono",            "referencia_telefono",  "tel"],
                                    ["Relación",            "referencia_relacion",  "text"],
                                    ["Tiempo de conocerse", "referencia_tiempo",    "text"],
                                ].map(([label, key, type]) => (
                                    <div key={key}>
                                        <Label className="text-xs uppercase tracking-wider text-slate-500">{label}</Label>
                                        <Input type={type} value={nuevoReferencia[key]}
                                            onChange={e => setNuevoReferencia({ ...nuevoReferencia, [key]: e.target.value })} />
                                    </div>
                                ))}
                            </div>
                            <Button type="button" onClick={referenciaAddRow}>Confirmar y Agregar</Button>
                        </DialogContent>
                    </Dialog>
                </CardContent>
            </Card>
        </div>
    );

    const S10 = (
        <div className="flex flex-col gap-4">
            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">Diagnóstico</CardTitle>
                </CardHeader>
                <CardContent>
                    <Textarea rows={6} value={data.diagnostico_desc} onChange={e => setData('diagnostico_desc', e.target.value)} placeholder="Diagnóstico socioeconómico..." />
                    <FieldError message={errors.diagnostico_desc} />
                </CardContent>
            </Card>
            <Card>
                <CardHeader className="pb-3">
                    <CardTitle className="text-sm font-bold text-slate-700 uppercase tracking-wide">Observaciones del trabajador social</CardTitle>
                </CardHeader>
                <CardContent>
                    <Textarea rows={5} value={data.obs_trabajadorSocial} onChange={e => setData('obs_trabajadorSocial', e.target.value)} placeholder="Observaciones..."/>
                    <FieldError message={errors.obs_trabajadorSocial} />
                </CardContent>
            </Card>
        </div>
    );

    const sectionContent = [S1, S2, S3, S4, S5, S6, S7, S8, S9, S10];

    // ── Render ────────────────────────────────────────────────────────────────

    return (
        <AppLayout breadcrumbs={breadcrumbs}>
            <form onSubmit={handleSubmit}>
                {/* Page header */}
                <div className="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between pb-4">
                    <div>
                        <h1 className="text-2xl font-bold tracking-tight text-slate-900">
                            {viendoExistente ? 'Ver estudio socioeconómico' : 'Registrar estudio socioeconómico'}
                        </h1>
                        <p className="text-sm text-slate-500 capitalize">{fechaHoy()}</p>
                    </div>
                    <div className="flex gap-2">
                        {viendoExistente ? (
                            <Button type="button" variant="outline" onClick={() => router.visit('/admisiones')}>Volver</Button>
                        ) : (
                            <>
                                <Button type="button" variant="outline">Cancelar</Button>
                                <Button type="submit" disabled={processing}>
                                    {processing ? <span className="animate-spin mr-2">⏳</span> : <Save size={14} className="mr-2" />}
                                    {processing ? "Guardando..." : "Guardar estudio"}
                                </Button>
                            </>
                        )}
                    </div>
                </div>

                {/* Errors / success */}
                {Object.keys(errors).length > 0 && (
                    <div className="mb-4 rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-700">
                        <p className="font-semibold mb-1">Por favor corrige los siguientes campos:</p>
                        <ul className="list-disc list-inside space-y-0.5">
                            {Object.values(errors).map((msg, i) => <li key={i}>{msg}</li>)}
                        </ul>
                    </div>
                )}
                {wasSuccessful && (
                    <div className="mb-4 flex items-center gap-2 rounded-xl border border-green-200 bg-green-50 p-4 text-sm text-green-700">
                        <CheckCircle2 size={16} /> Estudio socioeconómico guardado correctamente.
                    </div>
                )}

                {/* Main layout: sidebar + content */}
                <div className="flex gap-0 border border-slate-200 rounded-2xl overflow-hidden bg-white shadow-sm min-h-[600px]">

                    {/* Sidebar */}
                    <div className="w-52 flex-shrink-0 border-r border-slate-200 bg-slate-50 flex flex-col">
                        <div className="px-4 py-3 border-b border-slate-200">
                            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-slate-400">Secciones</p>
                        </div>
                        <nav className="flex flex-col flex-1 py-2">
                            {SECTION_TITLES.map((s, i) => (
                                <button
                                    key={i}
                                    type="button"
                                    onClick={() => setSection(i)}
                                    className={cn(
                                        "flex items-center gap-2.5 px-4 py-2.5 text-left text-xs transition-colors border-l-2",
                                        i === section
                                            ? "border-l-teal-600 bg-teal-50 text-teal-800 font-semibold"
                                            : i < section
                                                ? "border-l-teal-300 text-teal-700 hover:bg-slate-100"
                                                : "border-l-transparent text-slate-500 hover:bg-slate-100"
                                    )}
                                >
                                    <span className={cn(
                                        "w-5 h-5 rounded-full border flex items-center justify-center text-[10px] flex-shrink-0",
                                        i === section ? "border-teal-600 bg-teal-600 text-white" :
                                        i < section   ? "border-teal-500 bg-teal-500 text-white" :
                                                        "border-slate-300 text-slate-400"
                                    )}>
                                        {i < section ? "✓" : i + 1}
                                    </span>
                                    <span className="truncate">{s.label}</span>
                                </button>
                            ))}
                        </nav>
                        {/* Progress */}
                        <div className="px-4 py-3 border-t border-slate-200">
                            <div className="flex justify-between text-xs text-slate-400 mb-1">
                                <span>{section + 1} de {TOTAL_SECTIONS}</span>
                                <span>{progressPct}%</span>
                            </div>
                            <div className="h-1.5 w-full rounded-full bg-slate-200 overflow-hidden">
                                <div className="h-full bg-teal-500 transition-all duration-300" style={{ width: `${progressPct}%` }} />
                            </div>
                        </div>
                    </div>

                    {/* Content area */}
                    <div className="flex-1 flex flex-col overflow-hidden">
                        {/* Top bar */}
                        <div className="flex items-center justify-between px-6 py-3 border-b border-slate-200 bg-white">
                            <div>
                                <p className="text-sm font-semibold text-slate-800">{SECTION_TITLES[section].label}</p>
                                <p className="text-xs text-slate-400">Sección {section + 1} de {TOTAL_SECTIONS}</p>
                            </div>
                            <div className="flex gap-2">
                                {section > 0 && (
                                    <Button type="button" variant="outline" size="sm" onClick={() => setSection(s => s - 1)}>
                                        ← Anterior
                                    </Button>
                                )}
                                {section < TOTAL_SECTIONS - 1 ? (
                                    <Button type="button" size="sm" onClick={() => setSection(s => s + 1)}>
                                        Siguiente →
                                    </Button>
                                ) : (
                                    <Button type="submit" size="sm" disabled={processing}>
                                        <Save size={13} className="mr-1.5" />
                                        {processing ? "Guardando..." : "Guardar estudio"}
                                    </Button>
                                )}
                            </div>
                        </div>

                        {/* Section content */}
                        <fieldset
                            disabled={viendoExistente}
                            className={cn(
                                "flex-1 overflow-y-auto p-6",
                                viendoExistente && "opacity-80 [&_input]:cursor-default [&_select]:cursor-default [&_textarea]:cursor-default"
                            )}
                        >
                            {sectionContent[section]}
                        </fieldset>
                    </div>
                </div>
            </form>
        </AppLayout>
    );
}
