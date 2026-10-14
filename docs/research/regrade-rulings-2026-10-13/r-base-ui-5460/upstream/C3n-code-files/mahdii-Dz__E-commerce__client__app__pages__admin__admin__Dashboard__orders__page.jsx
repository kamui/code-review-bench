'use client';

import { useState, useMemo, useEffect, useRef, useCallback } from 'react';
import { Search, X, ChevronLeft, ChevronRight, Loader2, Edit, ChevronDown, ChevronUp, Eye, Truck, Phone, MessageCircle, Clock, PhoneOff, CheckCircle2, Timer, Package, XCircle, RotateCcw, RefreshCw, AlertTriangle, Loader, Plus } from 'lucide-react';
import Link from 'next/link';
import Image from 'next/image';
import { getThumbnailUrl } from '@/lib/imageUrl';
import { Select as BaseSelect } from '@base-ui/react/select';
import axios from 'axios';
import { useWilayaData } from '@/components/useWilayaData';
import { Input } from '@/components/ui/input';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import CustomSelect from '@/components/CustomSelect';

const ITEMS_PER_PAGE = 50;

const STATUSES = [
  { value: 'new', label: 'جديد', color: '#6366F1', icon: Clock },
  { value: 'didn\'t respond to the call number 1', label: 'لم يرد على الاتصال رقم 1', color: '#F97316', icon: PhoneOff },
  { value: 'didn\'t respond to the call number 2', label: 'لم يرد على الاتصال رقم 2', color: '#EA580C', icon: PhoneOff },
  { value: 'didn\'t respond to the call number 3', label: 'لم يرد على الاتصال رقم 3', color: '#DC2626', icon: PhoneOff },
  { value: 'confirmed', label: 'مؤكد', color: '#16A34A', icon: CheckCircle2 },
  { value: 'delayed', label: 'مؤجلة', color: '#CA8A04', icon: Timer },
  { value: 'Delivered', label: 'تم التوصيل', color: '#0891B2', icon: Package },
  { value: 'canceled by the shop', label: 'ملغي من المتجر', color: '#991B1B', icon: XCircle },
  { value: 'canceled by the customer', label: 'ملغي من الزبون', color: '#52525B', icon: XCircle },
  { value: 'returned', label: 'مرجعة', color: '#7C3AED', icon: RotateCcw },
];

const STATUS_MAP = Object.fromEntries(STATUSES.map(s => [s.value, s]));

function formatPhoneForWhatsApp(phone) {
  const digits = phone.replace(/[^\d]/g, '');
  if (digits.startsWith('0')) return '213' + digits.substring(1);
  if (digits.startsWith('213')) return digits;
  return '213' + digits;
}

function formatDate(dateStr) {
  if (!dateStr) return '-';
  const d = new Date(dateStr);
  return `${String(d.getDate()).padStart(2, '0')}/${String(d.getMonth() + 1).padStart(2, '0')}/${d.getFullYear()} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

function StatusSelect({ value, onChange, isUpdating }) {
  return <CustomSelect value={value} onChange={onChange} options={STATUSES} isUpdating={isUpdating} />;
}

const FILTER_TYPES = [
  { label: 'الكل', value: 'All' },
  { label: 'توصيل للمنزل', value: 'domicile' },
  { label: 'استلام من المكتب', value: 'stopDesk' },
];

export default function OrdersPage() {
  const { wilayaData } = useWilayaData();
  const [orders, setOrders] = useState([]);
  const [leftedCount, setLeftedCount] = useState(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState(null);
  const [currentPage, setCurrentPage] = useState(1);
  const [searchQuery, setSearchQuery] = useState('');
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState('');
  const debounceTimer = useRef(null);

  const handleSearchChange = (e) => {
    const val = e.target.value;
    setSearchQuery(val);
    setCurrentPage(1);
    if (debounceTimer.current) clearTimeout(debounceTimer.current);
    debounceTimer.current = setTimeout(() => setDebouncedSearchQuery(val), 300);
  };

  const clearSearch = () => {
    setSearchQuery('');
    setDebouncedSearchQuery('');
    setCurrentPage(1);
    if (debounceTimer.current) clearTimeout(debounceTimer.current);
  };

  const [toast, setToast] = useState({ show: false, message: '', type: 'success' });

  const [editingOrder, setEditingOrder] = useState(null);
  const [editForm, setEditForm] = useState({
    first_name: '',
    last_name: '',
    phone: '',
    wilaya: 'Alger',
    baladiya: '',
    delivery_type: 'domicile',
    delivery_Price: 0,
    wilaya_code: ''
  });
  const [editItems, setEditItems] = useState([]);
  const [isSaving, setIsSaving] = useState(false);
  const [productColorsMap, setProductColorsMap] = useState({});
  const [productOffersMap, setProductOffersMap] = useState({});

  const [viewingOrder, setViewingOrder] = useState(null);

  const [sendingToDelivery, setSendingToDelivery] = useState(new Set());
  const [updatingStatus, setUpdatingStatus] = useState(new Set());

  const [filters, setFilters] = useState({
    deliveryType: 'All',
    currentStatus: '',
    minPrice: '',
    maxPrice: '',
    wilaya: '',
    baladiya: '',
    startDate: '',
    endDate: '',
    product: '',
  });

  const [expandedOrders, setExpandedOrders] = useState(new Set());

  const scrollRef = useRef(null);
  const [canScrollRight, setCanScrollRight] = useState(false);
  const [canScrollLeft, setCanScrollLeft] = useState(false);

  const updateScrollButtons = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const maxScroll = el.scrollWidth - el.clientWidth;
    const scrollPos = Math.abs(el.scrollLeft);
    setCanScrollLeft(scrollPos < Math.abs(maxScroll) - 5);
    setCanScrollRight(scrollPos > 5);
  }, []);

  const scrollTable = (direction) => {
    const el = scrollRef.current;
    if (!el) return;
    const step = el.clientWidth * 0.8;
    el.scrollBy({ left: direction * step, behavior: 'smooth' });
  };

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.addEventListener('scroll', updateScrollButtons);
    updateScrollButtons();
    return () => el.removeEventListener('scroll', updateScrollButtons);
  }, [orders, updateScrollButtons]);

  const showToast = (message, type = 'success') => {
    setToast({ show: true, message, type });
    setTimeout(() => setToast({ show: false, message: '', type: 'success' }), 3000);
  };

  const toggleOrderExpansion = (orderId) => {
    setExpandedOrders(prev => {
      const newSet = new Set(prev);
      if (newSet.has(orderId)) newSet.delete(orderId);
      else newSet.add(orderId);
      return newSet;
    });
  };

  useEffect(() => {
    const fetchOrders = async () => {
      try {
        setLoading(true);
        const response = await axios.get('/api/shop/orders');
        setOrders(response.data);
        setError(null);
      } catch (err) {
        console.error('Error fetching orders:', err);
        setError('فشل تحميل الطلبات. حاول مرة أخرى.');
      } finally {
        setLoading(false);
      }
    };
    fetchOrders();
    fetch('/api/shop/lefted-orders')
      .then(res => res.ok ? res.json() : [])
      .then(data => setLeftedCount(Array.isArray(data) ? data.length : 0))
      .catch(() => setLeftedCount(0));
  }, []);

  const handleRefresh = useCallback(async () => {
    setRefreshing(true);
    try {
      const response = await axios.get('/api/shop/orders');
      setOrders(response.data);
      const leftedRes = await fetch('/api/shop/lefted-orders');
      const leftedData = await (leftedRes.ok ? leftedRes.json() : []);
      setLeftedCount(Array.isArray(leftedData) ? leftedData.length : 0);
    } catch (err) {
      console.error('Refresh failed:', err);
    } finally {
      setRefreshing(false);
      showToast('تم تحديث البيانات بنجاح','success' );
    }
  }, []);

  const filteredOrders = useMemo(() => {
    const list = orders || [];
    const q = debouncedSearchQuery.toLowerCase().trim();
    const dType = filters.deliveryType;
    const minP = filters.minPrice !== '' ? Number(filters.minPrice) : null;
    const maxP = filters.maxPrice !== '' ? Number(filters.maxPrice) : null;
    const wilayaQ = filters.wilaya.toLowerCase().trim();
    const baladiyaQ = filters.baladiya.toLowerCase().trim();
    const cStatus = filters.currentStatus;
    const startD = filters.startDate;
    const endD = filters.endDate;
    const prodQ = filters.product.toLowerCase().trim();
    const result = [];

    for (let i = 0; i < list.length; i++) {
      const order = list[i];

      if (q && !`${order.first_name} ${order.last_name}`.toLowerCase().includes(q) &&
          !order.phone?.includes(q) &&
          !order.items?.some(item => item.product_name.toLowerCase().includes(q)) &&
          !order.order_number?.toLowerCase().includes(q) &&
          !order.order_id?.toString().includes(q)) {
        continue;
      }

      if (dType !== 'All' && order.delivery_type !== dType) continue;

      if (minP !== null) {
        const price = Array.isArray(order.items) ? order.totalPrice : order.fullPrice;
        if (price < minP) continue;
      }
      if (maxP !== null) {
        const price = Array.isArray(order.items) ? order.totalPrice : order.fullPrice;
        if (price > maxP) continue;
      }

      if (wilayaQ && !order.wilaya?.toLowerCase().includes(wilayaQ)) continue;
      if (baladiyaQ && !order.baladiya?.toLowerCase().includes(baladiyaQ)) continue;
      if (cStatus && order.current_status !== cStatus) continue;

      if (startD) {
        const d = new Date(order.created_at);
        const orderDate = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
        if (orderDate < startD) continue;
      }
      if (endD) {
        const d = new Date(order.created_at);
        const orderDate = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
        if (orderDate > endD) continue;
      }

      if (prodQ && !order.items?.some(item => item.product_name.toLowerCase().includes(prodQ))) continue;

      result.push(order);
    }

    return result;
  }, [orders, debouncedSearchQuery, filters]);

  const statusCounts = useMemo(() => {
    const counts = {};
    for (const order of orders) {
      const s = order.current_status || 'new';
      counts[s] = (counts[s] || 0) + 1;
    }
    return counts;
  }, [orders]);

  const totalPages = Math.ceil(filteredOrders.length / ITEMS_PER_PAGE);
  const startIndex = (currentPage - 1) * ITEMS_PER_PAGE;
  const endIndex = startIndex + ITEMS_PER_PAGE;
  const currentOrders = filteredOrders.slice(startIndex, endIndex);

  const handleFilterChange = (key, value) => {
    setFilters(prev => ({ ...prev, [key]: value }));
    setCurrentPage(1);
  };

  const clearFilters = () => {
    setFilters({ deliveryType: 'All', currentStatus: '', minPrice: '', maxPrice: '', wilaya: '', baladiya: '', startDate: '', endDate: '', product: '' });
    setCurrentPage(1);
  };

  const hasActiveFilters =
    filters.deliveryType !== 'All' ||
    filters.currentStatus !== '' ||
    filters.minPrice !== '' ||
    filters.maxPrice !== '' ||
    filters.wilaya !== '' ||
    filters.baladiya !== '' ||
    filters.startDate !== '' ||
    filters.endDate !== '' ||
    filters.product !== '';

  const goToPage = (page) => {
    if (page >= 1 && page <= totalPages) setCurrentPage(page);
  };

  const getPageNumbers = () => {
    const pages = [];
    const maxVisiblePages = 5;
    if (totalPages <= maxVisiblePages) {
      for (let i = 1; i <= totalPages; i++) pages.push(i);
    } else {
      if (currentPage <= 3) {
        for (let i = 1; i <= 4; i++) pages.push(i);
        pages.push('...');
        pages.push(totalPages);
      } else if (currentPage >= totalPages - 2) {
        pages.push(1);
        pages.push('...');
        for (let i = totalPages - 3; i <= totalPages; i++) pages.push(i);
      } else {
        pages.push(1);
        pages.push('...');
        for (let i = currentPage - 1; i <= currentPage + 1; i++) pages.push(i);
        pages.push('...');
        pages.push(totalPages);
      }
    }
    return pages;
  };

  const renderPagination = (isTop) => (
    <footer className={`flex items-center justify-between ${isTop ? 'mb-4' : 'mt-6 pt-4 border-t border-gray-200'}`}>
      <span className="text-sm text-gray-600">
        عرض {filteredOrders.length > 0 ? startIndex + 1 : 0}-{Math.min(endIndex, filteredOrders.length)} من {filteredOrders.length} طلب
      </span>

      <div className="flex items-center gap-2">
        <button
          onClick={() => goToPage(currentPage - 1)}
          disabled={currentPage === 1}
          className="inline-flex cursor-pointer items-center gap-1 px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-600 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          <ChevronRight size={18} />
          <span>السابق</span>
        </button>

        <div className="flex gap-1">
          {getPageNumbers().map((page, index) => (
            <div key={index}>
              {page === '...' ? (
                <span className="w-8 h-8 flex items-center justify-center text-gray-600">...</span>
              ) : (
                <button
                  onClick={() => goToPage(page)}
                  className={`w-8 h-8 rounded-lg text-sm font-medium transition-colors ${currentPage === page ? 'bg-[#FA3145] text-white' : 'text-gray-600 hover:bg-gray-100'}`}
                >
                  {page}
                </button>
              )}
            </div>
          ))}
        </div>

        <button
          onClick={() => goToPage(currentPage + 1)}
          disabled={currentPage === totalPages || totalPages === 0}
          className="inline-flex cursor-pointer items-center gap-1 px-3 py-2 border border-gray-200 rounded-lg text-sm text-gray-600 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          <span>التالي</span>
          <ChevronLeft size={18} />
        </button>
      </div>
    </footer>
  );

  const handleStatusChange = async (orderId, newStatus) => {
    setUpdatingStatus(prev => new Set(prev).add(orderId));
    try {
      const response = await axios.put(`/api/shop/orders/${orderId}`, {
        current_status: newStatus
      });
      if (response.status === 200) {
        setOrders(prev => prev.map(o => o.order_id === orderId ? { ...o, current_status: newStatus } : o));
        const order = orders.find(o => o.order_id === orderId);
        showToast(`تم تحديث حالة الطلب ${order?.order_number || orderId}`, 'success');
      }
    } catch (err) {
      showToast(err.response?.data?.error || 'فشل تحديث الحالة', 'error');
    } finally {
      setUpdatingStatus(prev => { const n = new Set(prev); n.delete(orderId); return n; });
    }
  };

  const handleSendToDelivery = async (order) => {
    setSendingToDelivery(prev => new Set(prev).add(order.order_id));
    try {
      const totalQty = order.items?.reduce((sum, item) => sum + item.quantity, 0) || 0;
      const fullName = `${order.first_name || ''} ${order.last_name || ''}`.trim();
      const produit = order.items?.map(item => {
        const colorQtys = item.colors?.map(c => `${c.color_name} (${c.quantity})`).join(', ') || '';
        const pkgNaming = item.package_naming;
        if (pkgNaming?.enabled && pkgNaming.mainName) {
          let offerName = item.offer_text || '';
          if (item.offer_text && item.offer_data) {
            const match = item.offer_text.match(/(\d+)\s*for\s*(\d+)/);
            if (match) {
              const matchedOffer = item.offer_data.find(o => o.quantity === parseInt(match[1]) && o.price === parseFloat(match[2]));
              if (matchedOffer?.packageName) offerName = matchedOffer.packageName;
            }
          }
          return `${pkgNaming.mainName}${offerName ? ' - ' + offerName : ''} (${colorQtys})`;
        }
        return `${item.product_name}${item.offer_text ? ' - ' + item.offer_text : ''} (${colorQtys})`;
      }).join(', ');

      const rawWilayaCode = order.wilaya_code || Object.keys(wilayaData).find(key => wilayaData[key]?.name === order.wilaya) || '';
      const wilayaCode = rawWilayaCode ? String(parseInt(rawWilayaCode, 10)) : '';
      const response = await axios.post('/api/admin/Delivery', {
        order_id: order.order_id,
        reference: order.order_number,
        nom_client: fullName,
        telephone: order.phone,
        commune: order.baladiya,
        code_wilaya: wilayaCode,
        address: order.address,
        produit: produit,
        quantite: totalQty,
        montant: order.totalPrice,
        boutique: 'La Maison D\'or',
        delivery_type: order.delivery_type === 'domicile' ? 0 : 1
      });

      if (response.status === 200) {
        showToast(`تم إرسال الطلب ${order.order_number} للتوصيل بنجاح`, 'success');
        setOrders(prev => prev.map(o =>
          o.order_id === order.order_id ? { ...o, delivery_sent: true } : o
        ));
      } else if (response.data?.success === false || response.data?.error) {
        showToast(`فشل الإرسال: ${response.data.error}`, 'error');
      }
    } catch (error) {
      const errData = error.response?.data;
      if (errData?.errors?.commune) {
        showToast('هذه البلدية لا يدعمها التوصيل', 'error');
      } else {
        showToast(errData?.error || 'فشل إرسال الطلب للتوصيل', 'error');
      }
    } finally {
      setSendingToDelivery(prev => {
        const newSet = new Set(prev);
        newSet.delete(order.order_id);
        return newSet;
      });
    }
  };

  const editCommunes = useMemo(() => {
    const code = Object.keys(wilayaData).find(key => wilayaData[key]?.name === editForm.wilaya);
    return code ? wilayaData[code]?.municipalities || [] : [];
  }, [editForm.wilaya]);

  const handleEdit = async (order) => {
    setEditingOrder(order);
    const code = Object.keys(wilayaData).find(key => wilayaData[key]?.name === order.wilaya) || '';
    const baladiya = order.baladiya || '';
    setEditForm({
      first_name: order.first_name || '',
      last_name: order.last_name || '',
      phone: order.phone || '',
      wilaya: order.wilaya || 'Alger',
      baladiya,
      delivery_type: order.delivery_type || 'domicile',
      delivery_Price: order.delivery_Price || 0,
      wilaya_code: order.wilaya_code || code
    });
    setEditItems(JSON.parse(JSON.stringify(order.items || [])));

    const uniqueIds = [...new Set((order.items || []).map(i => i.product_id).filter(Boolean))];
    const colorsMap = {};
    const offersMap = {};
    await Promise.all(uniqueIds.map(async (pid) => {
      try {
        const res = await axios.get(`/api/shop/products/${pid}`);
        if (res.data) {
          colorsMap[pid] = res.data.colors || [];
          offersMap[pid] = res.data.offers || [];
        }
      } catch (e) {
        console.error(`Failed to fetch product ${pid}:`, e);
      }
    }));
    setProductColorsMap(colorsMap);
    setProductOffersMap(offersMap);
  };

  const handleEditItemToggleColor = (itemIdx, colorName, colorHex) => {
    setEditItems(prev => {
      const updated = [...prev];
      const item = { ...updated[itemIdx] };
      let colors = [...(item.colors || [])];
      const existingIdx = colors.findIndex(c => c.color_name === colorName);
      if (existingIdx >= 0) {
        colors.splice(existingIdx, 1);
      } else {
        colors.push({ color_name: colorName, color_hex: colorHex, quantity: 1 });
      }
      item.colors = colors;
      updated[itemIdx] = item;
      return updated;
    });
  };

  const handleEditWilayaChange = (value) => {
    const code = Object.keys(wilayaData).find(key => wilayaData[key]?.name === value);
    if (!code) return;
    const data = wilayaData[code];
    const newCommunes = data?.municipalities || [];
    const newBaladiya = newCommunes.length > 0 ? newCommunes[0] : '';
    setEditForm(prev => ({ ...prev, wilaya: value, wilaya_code: code, baladiya: newBaladiya }));
  };

  // Auto-calculate delivery price when wilaya or delivery type changes
  useEffect(() => {
    const code = Object.keys(wilayaData).find(key => wilayaData[key]?.name === editForm.wilaya);
    if (code) {
      const wilayaInfo = wilayaData[code];
      const autoPrice = editForm.delivery_type === 'domicile'
        ? (wilayaInfo.domicilePrice || 0)
        : (wilayaInfo.stopDeskPrice || 0);
      setEditForm(prev => ({ ...prev, delivery_Price: autoPrice }));
    }
  }, [editForm.wilaya, editForm.delivery_type, wilayaData]);

  const handleEditItemColorChange = (itemIdx, colorIdx, field, value) => {
    setEditItems(prev => {
      const updated = [...prev];
      const item = { ...updated[itemIdx] };
      const colors = [...(item.colors || [])];
      if (colorIdx >= colors.length) colors.push({ color_name: '', color_hex: '', quantity: 1 });
      colors[colorIdx] = { ...colors[colorIdx], [field]: value };
      item.colors = colors;
      updated[itemIdx] = item;
      return updated;
    });
  };

  const handleEditItemAddColor = (itemIdx) => {
    setEditItems(prev => {
      const updated = [...prev];
      const item = { ...updated[itemIdx] };
      item.colors = [...(item.colors || []), { color_name: '', color_hex: '', quantity: 1 }];
      updated[itemIdx] = item;
      return updated;
    });
  };

  const handleEditItemRemoveColor = (itemIdx, colorIdx) => {
    setEditItems(prev => {
      const updated = [...prev];
      const item = { ...updated[itemIdx] };
      const colors = [...(item.colors || [])];
      colors.splice(colorIdx, 1);
      item.colors = colors;
      updated[itemIdx] = item;
      return updated;
    });
  };

  const handleEditItemChange = (itemIdx, field, value) => {
    setEditItems(prev => {
      const updated = [...prev];
      updated[itemIdx] = { ...updated[itemIdx], [field]: value };
      return updated;
    });
  };

  const handleEditItemOfferChange = (itemIdx, selectedValue, offers) => {
    setEditItems(prev => {
      const updated = [...prev];
      const item = { ...updated[itemIdx] };
      if (!selectedValue) {
        item.offer_text = '';
      } else {
        const matched = offers.find(o => `${o.quantity} for ${o.price} DA` === selectedValue);
        if (matched) {
          item.offer_text = selectedValue;
          item.quantity = Number(matched.quantity) || 1;
        } else {
          item.offer_text = selectedValue;
        }
      }
      updated[itemIdx] = item;
      return updated;
    });
  };

  const handleSave = async () => {
    for (let i = 0; i < editItems.length; i++) {
      const item = editItems[i];
      const colors = item.colors || [];
      if (colors.length > 0) {
        const sumColors = colors.reduce((s, c) => s + (Number(c.quantity) || 0), 0);
        const expected = Number(item.quantity) || 0;
        if (sumColors !== expected) {
          showToast(`مجموع كميات الألوان (${sumColors}) لا يساوي الكمية الإجمالية (${expected}) للمنتج "${item.product_name}"`, 'error');
          return;
        }
      }
    }

    setIsSaving(true);
    try {
      const expandedItems = [];
      for (const item of editItems) {
        const colors = item.colors || [];
        if (colors.length > 0) {
          for (const c of colors) {
            expandedItems.push({
              product_id: item.product_id,
              quantity: Number(c.quantity) || 1,
              price_per_unit: item.price_per_unit,
              color_name: c.color_name || null,
              color_hex: c.color_hex || null,
              offer_text: item.offer_text || null
            });
          }
        } else {
          expandedItems.push({
            product_id: item.product_id,
            quantity: item.quantity,
            price_per_unit: item.price_per_unit,
            color_name: null,
            color_hex: null,
            offer_text: item.offer_text || null
          });
        }
      }

      const hasFreeDelivery = editItems.some(item => {
        const offers = productOffersMap[item.product_id] || [];
        return offers.some(o => {
          const label = `${o.quantity} for ${o.price} DA`;
          return (item.offer_text || '').startsWith(label) && o.freeDelivery;
        });
      });

      const response = await fetch(`/api/shop/orders/${editingOrder.order_id}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          ...editForm,
          free_delivery: hasFreeDelivery,
          items: expandedItems
        })
      });
      if (!response.ok) {
        const err = await response.json();
        throw new Error(err.error || 'فشل تحديث الطلب');
      }
      setEditingOrder(null);
      const refreshResponse = await axios.get('/api/shop/orders');
      setOrders(refreshResponse.data);
      showToast(`تم تحديث الطلب ${editingOrder.order_number} بنجاح`);
    } catch (err) {
      showToast(err.message || 'فشل تحديث الطلب', 'error');
    } finally {
      setIsSaving(false);
    }
  };

  if (loading) {
    return (
      <div className="w-full pt-6 px-9 pb-16 flex items-center justify-center h-96">
        <p className="text-gray-500">جار تحميل الطلبات...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="w-full pt-6 px-9 pb-16 flex items-center justify-center h-96">
        <p className="text-[#FA3145]">{error?.message || error}</p>
      </div>
    );
  }

  return (
    <div className="w-full pt-6 px-9 pb-16 relative">
      {toast.show && (
        <div className={`fixed top-6 right-6 z-[60] px-6 py-4 rounded-lg shadow-lg transition-all duration-300 ${toast.type === 'success' ? 'bg-green-500 text-white' : 'bg-red-500 text-white'}`}>
          <div className="flex items-center gap-3">
            {toast.type === 'success' ? (
              <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" /></svg>
            ) : (
              <X className="w-5 h-5" />
            )}
            <span className="font-medium">{toast.message}</span>
            <button onClick={() => setToast({ show: false, message: '', type: 'success' })} className="mr-2 hover:opacity-80">
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      <header className="flex items-center justify-between mb-8">
        <h1 className="text-3xl font-semibold text-black tracking-tight">جميع الطلبات</h1>
        <div className="flex items-center gap-2">
          <button onClick={handleRefresh} disabled={refreshing} className="p-2 hover:bg-gray-100 rounded-lg border border-stroke cursor-pointer transition-colors">
            <RefreshCw size={22} className={`text-gray-500 ${refreshing ? 'animate-spin' : ''}`} />
          </button>
          <Link
            href="/admin/add-order"
            className="inline-flex items-center gap-2 px-5 py-2.5 bg-[#FA3145] text-white rounded-xl hover:opacity-90 transition-opacity text-sm font-medium"
          >
            <Plus size={18} />
            إضافة طلب
          </Link>
        </div>
      </header>

      {leftedCount > 0 && (
        <div className="bg-amber-50 border-2 border-amber-200 rounded-xl p-4 w-full mb-6 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <AlertTriangle className="text-amber-600" size={24} />
            <div>
              <p className="text-amber-800 font-medium">لديك {leftedCount} طلب متروك</p>
              <p className="text-amber-600 text-sm">هناك طلبات لم يتم إكمالها من طرف الزبائن</p>
            </div>
          </div>
          <a href="/admin/lefted-orders"
            className="inline-flex items-center gap-2 px-4 py-2 bg-amber-600 text-white rounded-lg hover:bg-amber-700 transition-colors text-sm font-medium cursor-pointer">
            عرض الطلبات المتروكة
          </a>
        </div>
      )}

      <div className="bg-white border-2 border-stroke rounded-xl p-6 w-full mb-6">
        <h3 className="text-lg font-semibold text-gray-800 mb-4">إحصائيات الحالات</h3>
        <div className="flex flex-wrap gap-3">
          {STATUSES.map(status => {
            const count = statusCounts[status.value] || 0;
            const Icon = status.icon;
            return (
              <div
                key={status.value}
                className="flex items-center gap-2 px-4 py-2.5 rounded-xl border text-sm font-medium transition-shadow hover:shadow-sm"
                style={{
                  backgroundColor: `${status.color}15`,
                  color: status.color,
                  borderColor: `${status.color}30`,
                }}
              >
                <Icon size={16} />
                <span>{status.label}</span>
                <span className="font-bold tabular-nums">({count})</span>
              </div>
            );
          })}
        </div>
      </div>

      <div className="bg-white border-2 border-stroke rounded-xl p-6 w-full mb-6">
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">الحالة الحالية</label>
            <BaseSelect.Root dir="rtl"
              value={filters.currentStatus}
              onValueChange={(value) => handleFilterChange('currentStatus', value)}
            >
              <BaseSelect.Trigger className="flex w-full h-11 items-center justify-between rounded-xl border border-stroke bg-white px-4 text-right text-sm outline-none transition-colors focus:ring-2 focus:ring-[#FA3145]/20 focus:border-[#FA3145] data-[open]:ring-2 data-[open]:ring-[#FA3145]/20 data-[open]:border-[#FA3145]">
                <BaseSelect.Value placeholder="الكل">
                  {(value) => {
                    const s = STATUSES.find(st => st.value === value);
                    return s ? s.label : 'الكل';
                  }}
                </BaseSelect.Value>
                <ChevronDown size={16} className="text-gray-500 shrink-0" />
              </BaseSelect.Trigger>
              <BaseSelect.Portal>
                <BaseSelect.Positioner side="bottom" align="start" alignItemWithTrigger={false} className="z-50">
                  <BaseSelect.Popup className="max-h-60 overflow-y-auto rounded-xl border border-stroke bg-white py-2 shadow-lg"
                    style={{ width: 'var(--anchor-width)' }}>
                    <BaseSelect.List>
                      <BaseSelect.Item value=""
                        className="flex cursor-pointer [direction:rtl] items-center justify-between px-4 py-2.5 text-sm outline-none data-[highlighted]:bg-gray-100 data-[selected]:text-[#FA3145]">
                        <BaseSelect.ItemText>الكل</BaseSelect.ItemText>
                      </BaseSelect.Item>
                      {STATUSES.map(s => (
                        <BaseSelect.Item key={s.value} value={s.value}
                          className="flex cursor-pointer [direction:rtl] items-center justify-between px-4 py-2.5 text-sm outline-none data-[highlighted]:bg-gray-100 data-[selected]:text-[#FA3145]">
                          <div className="flex items-center gap-2">
                            <span className="w-2.5 h-2.5 rounded-full flex-shrink-0" style={{ backgroundColor: s.color }} />
                            <BaseSelect.ItemText>{s.label}</BaseSelect.ItemText>
                          </div>
                        </BaseSelect.Item>
                      ))}
                    </BaseSelect.List>
                  </BaseSelect.Popup>
                </BaseSelect.Positioner>
              </BaseSelect.Portal>
            </BaseSelect.Root>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">الولاية</label>
            <input type="text" placeholder="أدخل اسم الولاية..." value={filters.wilaya}
              onChange={(e) => handleFilterChange('wilaya', e.target.value)}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-[#FA3145]" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">البلدية</label>
            <input type="text" placeholder="أدخل اسم البلدية..." value={filters.baladiya}
              onChange={(e) => handleFilterChange('baladiya', e.target.value)}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-[#FA3145]" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">نوع التوصيل</label>
            <BaseSelect.Root dir="rtl"
              value={filters.deliveryType}
              onValueChange={(value) => handleFilterChange('deliveryType', value)}
            >
              <BaseSelect.Trigger className="flex w-full h-11 items-center justify-between rounded-xl border border-stroke bg-white px-4 text-right text-sm outline-none transition-colors focus:ring-2 focus:ring-[#FA3145]/20 focus:border-[#FA3145] data-[open]:ring-2 data-[open]:ring-[#FA3145]/20 data-[open]:border-[#FA3145]">
                <BaseSelect.Value placeholder="الكل">
                  {(value) => {
                    const t = FILTER_TYPES.find(f => f.value === value);
                    return t ? t.label : 'الكل';
                  }}
                </BaseSelect.Value>
                <ChevronDown size={16} className="text-gray-500 shrink-0" />
              </BaseSelect.Trigger>
              <BaseSelect.Portal>
                <BaseSelect.Positioner side="bottom" align="start" alignItemWithTrigger={false} className="z-50">
                  <BaseSelect.Popup className="rounded-xl border border-stroke bg-white py-2 shadow-lg"
                    style={{ width: 'var(--anchor-width)' }}>
                    <BaseSelect.List>
                      {FILTER_TYPES.map(t => (
                        <BaseSelect.Item key={t.value} value={t.value}
                          className="flex cursor-pointer [direction:rtl] items-center justify-between px-4 py-2.5 text-sm outline-none data-[highlighted]:bg-gray-100 data-[selected]:text-[#FA3145]">
                          <BaseSelect.ItemText>{t.label}</BaseSelect.ItemText>
                        </BaseSelect.Item>
                      ))}
                    </BaseSelect.List>
                  </BaseSelect.Popup>
                </BaseSelect.Positioner>
              </BaseSelect.Portal>
            </BaseSelect.Root>
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">تاريخ البداية</label>
            <input type="date" value={filters.startDate}
              onChange={(e) => handleFilterChange('startDate', e.target.value)}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-[#FA3145]" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">تاريخ النهاية</label>
            <input type="date" value={filters.endDate}
              onChange={(e) => handleFilterChange('endDate', e.target.value)}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-[#FA3145]" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">المنتج</label>
            <input type="text" placeholder="اسم المنتج..." value={filters.product}
              onChange={(e) => handleFilterChange('product', e.target.value)}
              className="w-full px-3 py-2 border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-[#FA3145]" />
          </div>
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">نطاق السعر</label>
            <div className="flex gap-2">
              <input type="number" placeholder="الحد الأدنى" value={filters.minPrice}
                onChange={(e) => handleFilterChange('minPrice', e.target.value)}
                className="w-1/2 px-3 py-2 border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-[#FA3145]" />
              <input type="number" placeholder="الحد الأقصى" value={filters.maxPrice}
                onChange={(e) => handleFilterChange('maxPrice', e.target.value)}
                className="w-1/2 px-3 py-2 border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-[#FA3145]" />
            </div>
          </div>
        </div>
        {hasActiveFilters && (
          <div className="flex justify-end mt-4">
            <button onClick={clearFilters} className="px-4 py-2 border border-gray-200 rounded-lg text-gray-700 hover:bg-gray-50 transition-colors text-sm cursor-pointer">
              مسح التصفية
            </button>
          </div>
        )}
      </div>

      <div className="bg-white border-2 border-stroke rounded-xl p-6 w-full">
        <div className="relative w-full mb-6">
          <Search className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400" size={20} />
          <input
            type="text"
            placeholder="بحث في الطلبات..."
            value={searchQuery}
              onChange={handleSearchChange}
            className="w-full pr-10 pl-10 py-2.5 bg-white border border-gray-200 rounded-lg focus:outline-none focus:ring-2 focus:ring-[#FA3145] focus:border-transparent"
          />
          {searchQuery && (
            <button
              onClick={clearSearch}
              className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 cursor-pointer"
            >
              <X size={18} />
            </button>
          )}
        </div>
        {renderPagination(true)}
        <div ref={scrollRef} onScroll={updateScrollButtons} className="hidden md:block overflow-x-auto rounded-xl border border-gray-200 max-w-full" style={{ boxSizing: 'border-box' }}>
            <table className="w-full">
              <thead className="bg-gray-50 border-b border-gray-200">
                <tr>
                  <th className="px-4 py-4 text-right text-sm font-semibold text-gray-700 whitespace-nowrap">رقم الطلب</th>
                  <th className="px-4 py-4 text-right text-sm font-semibold text-gray-700 whitespace-nowrap">المنتج</th>
                  <th className="px-4 py-4 text-right text-sm font-semibold text-gray-700 whitespace-nowrap">الزبون</th>
                  <th className="px-4 py-4 text-right text-sm font-semibold text-gray-700 whitespace-nowrap">الموقع</th>
                  <th className="px-4 py-4 text-right text-sm font-semibold text-gray-700 whitespace-nowrap">الحالة الحالية</th>
                  <th className="px-4 py-4 text-right text-sm font-semibold text-gray-700 whitespace-nowrap">السعر</th>
                  <th className="px-4 py-4 text-right text-sm font-semibold text-gray-700 whitespace-nowrap">التاريخ</th>
                  <th className="px-4 py-4 text-center text-sm font-semibold text-gray-700 whitespace-nowrap">الإجراءات</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100">
                {currentOrders.map((order) => {
                  const statusInfo = STATUS_MAP[order.current_status] || STATUS_MAP['new'];
                  const isSending = sendingToDelivery.has(order.order_id);
                  const waPhone = formatPhoneForWhatsApp(order.phone || '');

                  return (
                    <tr key={order.order_id} className="hover:bg-gray-50 transition-colors">
                      <td className="px-4 py-4 text-sm font-bold text-gray-900 whitespace-nowrap font-mono">
                        {order.order_number || `#${order.order_id}`}
                      </td>
                      <td className="px-4 py-4 text-sm text-gray-600">
                        {order.items && order.items.length > 0 ? (
                          <div className="flex flex-col gap-2.5">
                            {order.items.map((item, idx) => (
                              <div key={`${item.product_id}-${idx}`} className="text-sm flex items-start gap-2.5 min-w-0">
                                <div className="w-10 h-10 rounded-lg border border-gray-200 bg-gray-50 overflow-hidden flex-shrink-0">
                                  {item.thumbnail ? (
                                    <Image src={getThumbnailUrl(item.thumbnail)} alt={item.product_name} width={40} height={40} className="w-full h-full object-cover" loading="lazy" />
                                  ) : (
                                    <div className="w-full h-full flex items-center justify-center text-gray-400">
                                      <Package size={16} />
                                    </div>
                                  )}
                                </div>
                                <div className="min-w-0 flex-1">
                                  <div className="flex flex-wrap items-baseline gap-x-2">
                                    <span className="font-medium text-gray-900 whitespace-nowrap">{item.product_name}</span>
                                    {item.offer_text && <span className="text-gray-500 text-xs">عرض: {item.offer_text}</span>}
                                  </div>
                                  {item.colors && item.colors.length > 0 && (
                                    <div className="grid grid-cols-[repeat(4,max-content)] gap-1.5 mt-1.5">
                                      {item.colors.map((c, ci) => (
                                        <span
                                          key={ci}
                                          className="inline-flex items-center gap-1 text-xs bg-gray-100 rounded-full px-2 py-0.5 whitespace-nowrap"
                                        >
                                          <span className="w-3 h-3 rounded-full border border-gray-300 flex-shrink-0" style={{ backgroundColor: `#${c.color_hex}` }} />
                                          <span>{c.color_name} {c.quantity}</span>
                                        </span>
                                      ))}
                                    </div>
                                  )}
                                </div>
                              </div>
                            ))}
                          </div>
                        ) : (
                          <span className="text-gray-400">-</span>
                        )}
                      </td>
                      <td className="px-4 py-4 text-sm whitespace-nowrap">
                        <div className="flex flex-col">
                          <span className="font-medium text-gray-900">{order.first_name} {order.last_name}</span>
                          <div className="flex items-center gap-2 mt-1">
                            <span className="text-gray-500" dir="ltr">{order.phone}</span>
                            <a href={`tel:${order.phone}`} className="text-blue-500 hover:text-blue-700" title="اتصال">
                              <Phone size={14} />
                            </a>
                            <a href={`https://wa.me/${waPhone}`} target="_blank" rel="noopener noreferrer" className="text-green-500 hover:text-green-700" title="واتساب">
                              <MessageCircle size={14} />
                            </a>
                          </div>
                        </div>
                      </td>
                      <td className="px-4 py-4 text-sm whitespace-nowrap">
                        <div className="flex flex-col">
                          <span className="text-gray-900 font-medium">{order.wilaya} - {order.baladiya}</span>
                          <span className="text-gray-500 text-xs mt-1">
                            {order.delivery_type === 'domicile' ? 'توصيل للمنزل' : 'استلام من المكتب'}
                          </span>
                          {order.free_delivery && (
                            <span className="text-green-600 text-xs mt-0.5 font-medium">توصيل مجاني</span>
                          )}
                        </div>
                      </td>
                      <td className="px-4 py-4 whitespace-nowrap">
                        <StatusSelect
                          value={order.current_status || 'new'}
                          onChange={(val) => handleStatusChange(order.order_id, val)}
                          isUpdating={updatingStatus.has(order.order_id)}
                        />
                      </td>
                      <td className="px-4 py-4 text-sm font-bold text-gray-900 whitespace-nowrap">
                        {order.totalPrice?.toFixed(2) || '0.00'} دج
                      </td>
                      <td className="px-4 py-4 text-sm text-gray-500 whitespace-nowrap">
                        {formatDate(order.created_at)}
                      </td>
                      <td className="px-4 py-4 text-center whitespace-nowrap">
                        <div className="flex items-center justify-center gap-2">
                          <button
                            onClick={() => setViewingOrder(order)}
                            className="p-1.5 hover:bg-gray-100 rounded-lg cursor-pointer transition-colors"
                            title="عرض التفاصيل"
                          >
                            <Eye className="w-5 h-5 text-gray-900" />
                          </button>
                          <button
                            onClick={() => handleEdit(order)}
                            className="p-1.5 hover:bg-gray-100 rounded-lg cursor-pointer transition-colors"
                            title="تعديل"
                          >
                            <Edit className="w-5 h-5 text-blue-600" />
                          </button>
                          {order.delivery_sent ? (
                            <span className="inline-flex items-center gap-1 text-xs text-green-600 font-medium px-2 py-1" title="تم الإرسال للتوصيل">
                              <CheckCircle2 size={16} />
                            </span>
                          ) : (
                            <button
                              onClick={() => handleSendToDelivery(order)}
                              disabled={isSending}
                              className={`p-1.5 rounded-lg transition-colors ${isSending ? 'cursor-wait' : 'hover:bg-gray-100 cursor-pointer'}`}
                              title="إرسال للتوصيل"
                            >
                              {isSending ? (
                                <Loader2 className="w-5 h-5 text-orange-500 animate-spin" />
                              ) : (
                                <Truck className="w-5 h-5 text-orange-500" />
                              )}
                            </button>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          {currentOrders.length > 0 && (
            <div className="sticky bottom-0 bg-white z-10 border-t border-gray-200 px-2 py-2">
              <div className="flex items-center justify-between">
                <button
                  onClick={() => scrollTable(1)}
                  disabled={!canScrollRight}
                  className="w-10 h-10 rounded-full bg-white shadow-md border border-gray-200 flex items-center justify-center hover:bg-gray-50 cursor-pointer disabled:opacity-20 disabled:cursor-not-allowed transition-all"
                >
                  <ChevronRight className="w-5 h-5 text-gray-600" />
                </button>
                <button
                  onClick={() => scrollTable(-1)}
                  disabled={!canScrollLeft}
                  className="w-10 h-10 rounded-full bg-white shadow-md border border-gray-200 flex items-center justify-center hover:bg-gray-50 cursor-pointer disabled:opacity-20 disabled:cursor-not-allowed transition-all"
                >
                  <ChevronLeft className="w-5 h-5 text-gray-600" />
                </button>
              </div>
            </div>
          )}

          <div className="md:hidden space-y-4">
            {currentOrders.map((order) => {
              const statusInfo = STATUS_MAP[order.current_status] || STATUS_MAP['new'];
              const isSending = sendingToDelivery.has(order.order_id);
              const waPhone = formatPhoneForWhatsApp(order.phone || '');

              return (
                <Card key={order.order_id} className="bg-white border-2 border-stroke">
                  <CardHeader className="pb-3">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2 flex-wrap">
                        <CardTitle className="text-base font-semibold text-gray-900">
                          {order.order_number || `#${order.order_id}`}
                        </CardTitle>
                        <span className="text-sm font-bold text-gray-900">
                          {order.totalPrice?.toFixed(2) || '0.00'} دج
                        </span>
                      </div>
                      <button
                        onClick={() => toggleOrderExpansion(order.order_id)}
                        className="p-3 hover:bg-gray-100 rounded-full transition-colors touch-manipulation min-h-[44px] min-w-[44px] flex items-center justify-center"
                      >
                        {expandedOrders.has(order.order_id) ? (
                          <ChevronUp className="w-5 h-5 text-gray-600" />
                        ) : (
                          <ChevronDown className="w-5 h-5 text-gray-600" />
                        )}
                      </button>
                    </div>
                  </CardHeader>

                  <CardContent className="pt-0 space-y-4">
                    <div className="flex items-center justify-between text-sm">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="font-medium text-gray-900">{order.first_name} {order.last_name}</span>
                        <span className={`inline-block w-2 h-2 rounded-full`} style={{ backgroundColor: statusInfo.color }} />
                        <span className="text-gray-600" style={{ color: statusInfo.color }}>{statusInfo.label}</span>
                      </div>
                    </div>

                    {expandedOrders.has(order.order_id) && (
                      <div className="border-t border-gray-200 pt-4 space-y-4">
                        <div className="space-y-2">
                          <div className="flex items-start gap-2 text-sm">
                            <span className="text-gray-600 min-w-[80px]">الهاتف:</span>
                            <div className="flex items-center gap-2 flex-wrap">
                              <span className="text-gray-900" dir="ltr">{order.phone || '-'}</span>
                              <a href={`tel:${order.phone}`} className="text-blue-500 hover:text-blue-700"><Phone size={14} /></a>
                              <a href={`https://wa.me/${waPhone}`} target="_blank" rel="noopener noreferrer" className="text-green-500 hover:text-green-700"><MessageCircle size={14} /></a>
                            </div>
                          </div>
                          <div className="flex items-start gap-2 text-sm">
                            <span className="text-gray-600 min-w-[80px]">الولاية:</span>
                            <span className="text-gray-900">{order.wilaya || '-'}</span>
                          </div>
                          <div className="flex items-start gap-2 text-sm">
                            <span className="text-gray-600 min-w-[80px]">البلدية:</span>
                            <span className="text-gray-900">{order.baladiya || '-'}</span>
                          </div>
                          <div className="flex items-start gap-2 text-sm">
                            <span className="text-gray-600 min-w-[80px]">التوصيل:</span>
                            <span className="text-gray-900">{order.delivery_type === 'domicile' ? 'توصيل للمنزل' : 'استلام من المكتب'}{order.free_delivery ? <span className="text-green-600 text-xs mr-1 font-medium">(توصيل مجاني)</span> : ''}</span>
                          </div>
                          <div className="flex items-start gap-2 text-sm">
                            <span className="text-gray-600 min-w-[80px]">الحالة:</span>
                            <StatusSelect
                              value={order.current_status || 'new'}
                              onChange={(val) => handleStatusChange(order.order_id, val)}
                              isUpdating={updatingStatus.has(order.order_id)}
                            />
                          </div>
                        </div>

                        <div className="space-y-3">
                          <h4 className="text-sm font-semibold text-gray-900">المنتجات</h4>
                          {order.items && order.items.length > 0 ? (
                            <div className="space-y-3">
                              {order.items.map((item, idx) => (
                                <div key={`${item.product_id}-${idx}`} className="bg-gray-50 rounded-lg p-3">
                                  <div className="flex items-start gap-2.5 mb-2">
                                    <div className="w-10 h-10 rounded-lg border border-gray-200 bg-gray-50 overflow-hidden flex-shrink-0">
                                      {item.thumbnail ? (
                                        <Image src={getThumbnailUrl(item.thumbnail)} alt={item.product_name} width={40} height={40} className="w-full h-full object-cover" loading="lazy" />
                                      ) : (
                                        <div className="w-full h-full flex items-center justify-center text-gray-400">
                                          <Package size={16} />
                                        </div>
                                      )}
                                    </div>
                                    <div className="font-medium text-sm text-gray-900 min-w-0">
                                      {item.product_name}
                                      {item.offer_text && <span className="text-xs text-gray-600 block">عرض: {item.offer_text}</span>}
                                    </div>
                                  </div>
                                  {item.colors && item.colors.length > 0 && (
                                    <div className="flex flex-wrap gap-2 mb-2">
                                      {item.colors.map((c, ci) => (
                                        <div key={ci} className="flex items-center gap-1.5 bg-white rounded-full pl-1 pr-2 py-0.5 border border-gray-200">
                                          <div className="w-4 h-4 rounded-full border border-gray-300" style={{ backgroundColor: `#${c.color_hex}` }} />
                                          <span className="text-xs text-gray-700">{c.color_name} ({c.quantity})</span>
                                        </div>
                                      ))}
                                    </div>
                                  )}
                                  <div className="flex items-center justify-between text-xs text-gray-600">
                                    <span>الكمية: {item.quantity}</span>
                                    <span className="font-semibold text-gray-900">
                                      {Number(item.fullPrice || item.price_per_unit * item.quantity).toFixed(2)} دج
                                    </span>
                                  </div>
                                </div>
                              ))}
                            </div>
                          ) : (
                            <p className="text-sm text-gray-400">لا توجد منتجات</p>
                          )}
                        </div>

                        <div className="flex gap-2 pt-2 border-t border-gray-200">
                          <button
                            onClick={() => setViewingOrder(order)}
                            className="flex-1 flex items-center justify-center gap-1 px-2 py-3 rounded-lg border-2 border-gray-400 hover:bg-gray-50 transition-colors text-xs font-medium text-gray-600 touch-manipulation min-h-[44px]"
                          >
                            <Eye size={16} /> عرض
                          </button>
                          <button
                            onClick={() => handleEdit(order)}
                            className="flex-1 flex items-center justify-center gap-1 px-2 py-3 rounded-lg border-2 border-blue-500 hover:bg-blue-50 transition-colors text-xs font-medium text-blue-600 touch-manipulation min-h-[44px]"
                          >
                            <Edit size={16} /> تعديل
                          </button>
                          {order.delivery_sent ? (
                            <div className="flex-1 flex items-center justify-center gap-1 px-2 py-3 rounded-lg border-2 border-green-500 bg-green-50 text-green-600 text-xs font-medium">
                              <CheckCircle2 size={16} /> تم الإرسال
                            </div>
                          ) : (
                            <button
                              onClick={() => handleSendToDelivery(order)}
                              disabled={isSending}
                              className={`flex-1 flex items-center justify-center gap-1 px-2 py-3 rounded-lg border-2 transition-colors text-xs font-medium touch-manipulation min-h-[44px] ${isSending ? 'border-orange-400 bg-orange-50 cursor-wait text-orange-600' : 'border-orange-500 hover:bg-orange-50 text-orange-600'}`}
                            >
                              {isSending ? <Loader2 size={16} className="animate-spin" /> : <Truck size={16} />} توصيل
                            </button>
                          )}
                        </div>
                      </div>
                    )}
                  </CardContent>
                </Card>
              );
            })}
            {currentOrders.length === 0 && (
              <div className="text-center py-12">
                <p className="text-gray-500">لا توجد طلبات تطابق التصفية</p>
              </div>
            )}
          </div>

        {renderPagination(false)}
      </div>

      {/* View Modal */}
      {viewingOrder && (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4" onClick={() => setViewingOrder(null)}>
          <div className="bg-white rounded-xl w-full max-w-3xl max-h-[90vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
            <div className="p-6 border-b border-gray-200 flex justify-between items-center">
              <h2 className="text-xl font-bold text-gray-800">
                تفاصيل الطلب {viewingOrder.order_number || `#${viewingOrder.order_id}`}
              </h2>
              <button onClick={() => setViewingOrder(null)} className="text-gray-500 hover:text-gray-700">
                <X size={24} />
              </button>
            </div>
            <div className="p-6 space-y-6">
              <div className="flex items-center gap-3">
                <span className="text-sm font-semibold">الحالة:</span>
                <span className="px-3 py-1 rounded-full text-sm font-medium" style={{
                  backgroundColor: `${(STATUS_MAP[viewingOrder.current_status] || STATUS_MAP['new']).color}15`,
                  color: (STATUS_MAP[viewingOrder.current_status] || STATUS_MAP['new']).color
                }}>
                  {(STATUS_MAP[viewingOrder.current_status] || STATUS_MAP['new']).label}
                </span>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm text-gray-500 mb-1">الاسم الكامل</label>
                  <p className="font-medium text-gray-900">{viewingOrder.first_name} {viewingOrder.last_name}</p>
                </div>
                <div>
                  <label className="block text-sm text-gray-500 mb-1">الهاتف</label>
                  <div className="flex items-center gap-2">
                    <span className="text-gray-900" dir="ltr">{viewingOrder.phone}</span>
                    <a href={`tel:${viewingOrder.phone}`} className="inline-flex items-center gap-1 text-blue-600 hover:text-blue-800 text-xs"><Phone size={14} /> اتصال</a>
                    <a href={`https://wa.me/${formatPhoneForWhatsApp(viewingOrder.phone || '')}`} target="_blank" rel="noopener noreferrer" className="inline-flex items-center gap-1 text-green-600 hover:text-green-800 text-xs"><MessageCircle size={14} /> واتساب</a>
                  </div>
                </div>
                <div>
                  <label className="block text-sm text-gray-500 mb-1">الولاية</label>
                  <p className="font-medium text-gray-900">{viewingOrder.wilaya || '-'}</p>
                </div>
                <div>
                  <label className="block text-sm text-gray-500 mb-1">البلدية</label>
                  <p className="font-medium text-gray-900">{viewingOrder.baladiya || '-'}</p>
                </div>
                <div>
                  <label className="block text-sm text-gray-500 mb-1">نوع التوصيل</label>
                  <p className="font-medium text-gray-900">{viewingOrder.delivery_type === 'domicile' ? 'توصيل للمنزل' : 'استلام من المكتب'}{viewingOrder.free_delivery ? <span className="text-green-600 text-xs mr-2 font-medium">(توصيل مجاني)</span> : ''}</p>
                </div>
                <div>
                  <label className="block text-sm text-gray-500 mb-1">سعر التوصيل</label>
                  <p className="font-medium text-gray-900">{viewingOrder.free_delivery ? '0' : (viewingOrder.delivery_Price || 0)} دج</p>
                </div>
                <div>
                  <label className="block text-sm text-gray-500 mb-1">المجموع</label>
                  <p className="font-bold text-lg text-gray-900">{viewingOrder.totalPrice?.toFixed(2) || '0.00'} دج</p>
                </div>
                <div>
                  <label className="block text-sm text-gray-500 mb-1">تاريخ الطلب</label>
                  <p className="font-medium text-gray-900">{formatDate(viewingOrder.created_at)}</p>
                </div>
              </div>

              <div>
                <h4 className="font-semibold text-gray-800 mb-3">المنتجات</h4>
                <div className="border border-gray-200 rounded-lg overflow-hidden">
                  <table className="w-full text-sm">
                    <thead className="bg-gray-50">
                      <tr>
                        <th className="px-4 py-2 text-right text-xs font-medium text-gray-500">المنتج</th>
                        <th className="px-4 py-2 text-right text-xs font-medium text-gray-500">اللون</th>
                        <th className="px-4 py-2 text-right text-xs font-medium text-gray-500">الكمية</th>
                        <th className="px-4 py-2 text-right text-xs font-medium text-gray-500">السعر</th>
                        <th className="px-4 py-2 text-right text-xs font-medium text-gray-500">المجموع</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-gray-200">
                      {viewingOrder.items?.map((item, idx) => (
                        <tr key={`${item.product_id}-${idx}`}>
                          <td className="px-4 py-3">
                            <span className="font-medium">{item.product_name}</span>
                            {item.offer_text && <span className="text-xs text-gray-500 mr-1">عرض: {item.offer_text}</span>}
                          </td>
                          <td className="px-4 py-3">
                            {item.colors && item.colors.length > 0 ? (
                              <div className="flex flex-col gap-1">
                                {item.colors.map((c, ci) => (
                                  <div key={ci} className="flex items-center gap-2">
                                    <div className="w-4 h-4 rounded-full border border-gray-300 flex-shrink-0" style={{ backgroundColor: `#${c.color_hex}` }} />
                                    <span className="text-xs">{c.color_name} ({c.quantity})</span>
                                  </div>
                                ))}
                              </div>
                            ) : <span className="text-xs text-gray-400">-</span>}
                          </td>
                          <td className="px-4 py-3">{item.quantity}</td>
                          <td className="px-4 py-3">{Number(item.price_per_unit).toFixed(2)} دج</td>
                          <td className="px-4 py-3 font-medium">{Number(item.fullPrice || item.price_per_unit * item.quantity).toFixed(2)} دج</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
            <div className="p-6 border-t border-gray-200 flex justify-end">
              <button onClick={() => setViewingOrder(null)} className="px-6 py-2 bg-gray-100 rounded-lg text-gray-700 hover:bg-gray-200 transition-colors">إغلاق</button>
            </div>
          </div>
        </div>
      )}

      {/* Edit Modal */}
      {editingOrder && (
        <div className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4">
          <div className="bg-white rounded-xl w-full max-w-3xl max-h-[90vh] overflow-y-auto">
            <div className="p-6 border-b border-gray-200 flex justify-between items-center">
              <h2 className="text-xl font-bold text-gray-800">
                تعديل الطلب {editingOrder.order_number || `#${editingOrder.order_id}`}
              </h2>
              <button onClick={() => setEditingOrder(null)} disabled={isSaving} className="text-gray-500 hover:text-gray-700">
                <X size={24} />
              </button>
            </div>

            <div className="p-6 space-y-6">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">الاسم</label>
                  <Input value={editForm.first_name} onChange={(e) => setEditForm({ ...editForm, first_name: e.target.value })} className="w-full" />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">اللقب</label>
                  <Input value={editForm.last_name} onChange={(e) => setEditForm({ ...editForm, last_name: e.target.value })} className="w-full" />
                </div>
              </div>

              <div>
                <label className="block text-sm font-medium text-gray-700 mb-2">الهاتف</label>
                <Input value={editForm.phone} onChange={(e) => setEditForm({ ...editForm, phone: e.target.value })} className="w-full" />
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">الولاية</label>
                  <Select value={editForm.wilaya} onValueChange={handleEditWilayaChange}>
                    <SelectTrigger className="w-full h-11">
                      <SelectValue placeholder="اختر الولاية" />
                    </SelectTrigger>
                    <SelectContent>
                      {Object.entries(wilayaData || {})
                        .sort(([codeA], [codeB]) => parseInt(codeA, 10) - parseInt(codeB, 10))
                        .map(([code, data]) => (
                          <SelectItem key={code} value={data.name}>{code} - {data.name}</SelectItem>
                        ))}
                    </SelectContent>
                  </Select>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">البلدية</label>
                  <Select value={editForm.baladiya} onValueChange={(value) => setEditForm({ ...editForm, baladiya: value })} disabled={!editCommunes.length}>
                    <SelectTrigger className="w-full h-11">
                      <SelectValue placeholder="اختر البلدية" />
                    </SelectTrigger>
                    <SelectContent>
                      {editCommunes.map((commune) => (
                        <SelectItem key={commune} value={commune}>{commune}</SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </div>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">نوع التوصيل</label>
                  <Select value={editForm.delivery_type} onValueChange={(value) => setEditForm({ ...editForm, delivery_type: value })}>
                    <SelectTrigger className="w-full h-11">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value="domicile">توصيل للمنزل</SelectItem>
                      <SelectItem value="stopDesk">استلام من المكتب</SelectItem>
                    </SelectContent>
                  </Select>
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-700 mb-2">سعر التوصيل (دج)</label>
                  <Input type="number" value={editForm.delivery_Price} onChange={(e) => setEditForm({ ...editForm, delivery_Price: Number(e.target.value) })} className="w-full" min="0" />
                </div>
              </div>

              <div>
                <h4 className="font-semibold text-gray-800 mb-3">عناصر الطلب</h4>
                <div className="space-y-4">
                  {editItems.map((item, itemIdx) => {
                    const availColors = productColorsMap[item.product_id] || [];
                    const availOffers = productOffersMap[item.product_id] || [];
                    const hasProductColors = availColors.length > 0;
                    const hasProductOffers = availOffers.length > 0;
                    return (
                    <div key={item.item_id || itemIdx} className="border border-gray-200 rounded-lg p-4">
                      <div className="flex items-start justify-between mb-3">
                        <div className="flex-1">
                          <span className="font-medium text-sm text-gray-900">{item.product_name}</span>
                          {hasProductOffers ? (
                            <select
                              value={item.offer_text || ''}
                              onChange={(e) => handleEditItemOfferChange(itemIdx, e.target.value, availOffers)}
                              className="mt-1 block w-full text-xs px-2 py-1 border border-gray-200 rounded focus:outline-none focus:ring-1 focus:ring-[#FA3145] bg-white"
                            >
                              <option value="">بدون عرض</option>
                              {(() => {
                                const currentVal = item.offer_text || '';
                                const baseOptions = availOffers.map(o => `${o.quantity} for ${o.price} DA`);
                                const matchesExisting = currentVal === '' || baseOptions.some(b => currentVal.startsWith(b));
                                const options = [];
                                if (currentVal && !matchesExisting) {
                                  options.push(<option key="__fallback" value={currentVal}>{currentVal}</option>);
                                }
                                availOffers.forEach((offer, oi) => {
                                  const base = `${offer.quantity} for ${offer.price} DA`;
                                  const display = offer.freeDelivery ? `${base} (توصيل مجاني)` : base;
                                  options.push(<option key={oi} value={base}>{display}</option>);
                                });
                                return options;
                              })()}
                            </select>
                          ) : (
                            <input
                              type="text"
                              value={item.offer_text || ''}
                              onChange={(e) => handleEditItemChange(itemIdx, 'offer_text', e.target.value)}
                              placeholder="نص العرض"
                              className="mt-1 block w-full text-xs px-2 py-1 border border-gray-200 rounded focus:outline-none focus:ring-1 focus:ring-[#FA3145]"
                            />
                          )}
                        </div>
                        <div className="flex items-center gap-2 mr-3">
                          <label className="text-xs text-gray-500">الكمية:</label>
                          <div className="flex items-center gap-1">
                            <input
                              type="number"
                              value={item.quantity ?? ''}
                              onChange={(e) => handleEditItemChange(itemIdx, 'quantity', Number(e.target.value))}
                              disabled={!!item.offer_text}
                              className={`w-16 text-xs px-2 py-1 border border-gray-200 rounded focus:outline-none focus:ring-1 focus:ring-[#FA3145] ${item.offer_text ? 'bg-gray-100 text-gray-400 cursor-not-allowed' : ''}`}
                              min="1"
                            />
                            {item.offer_text && <span className="text-xs text-gray-400 whitespace-nowrap">(تلقائي)</span>}
                          </div>
                        </div>
                      </div>

                      <div className="space-y-2">
                        {hasProductColors ? (
                          <div className="flex flex-wrap gap-2">
                            {availColors.map((pc, ci) => {
                              const isSelected = (item.colors || []).some(c => c.color_name === pc.name);
                              const selectedColor = (item.colors || []).find(c => c.color_name === pc.name);
                              return (
                                <div key={ci} className="flex items-center gap-1">
                                  <button
                                    onClick={() => handleEditItemToggleColor(itemIdx, pc.name, pc.hex)}
                                    className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs border cursor-pointer transition-colors ${
                                      isSelected
                                        ? 'bg-[#FA3145] text-white border-[#FA3145]'
                                        : 'bg-white text-gray-700 border-gray-300 hover:border-gray-400'
                                    }`}
                                  >
                                    <span className="w-3.5 h-3.5 rounded-full border border-white/30 flex-shrink-0" style={{ backgroundColor: `#${pc.hex}` }} />
                                    <span>{pc.name}</span>
                                    {isSelected && <X size={12} className="mr-1" />}
                                  </button>
                                  {isSelected && (
                                    <input
                                      type="number"
                                      value={selectedColor?.quantity ?? 1}
                                      onChange={(e) => {
                                        const ci2 = (item.colors || []).findIndex(c => c.color_name === pc.name);
                                        if (ci2 >= 0) handleEditItemColorChange(itemIdx, ci2, 'quantity', Number(e.target.value));
                                      }}
                                      className="w-12 text-xs px-1 py-1 border border-gray-200 rounded focus:outline-none focus:ring-1 focus:ring-[#FA3145]"
                                      min="1"
                                    />
                                  )}
                                </div>
                              );
                            })}
                          </div>
                        ) : (
                          <>
                            {(item.colors || []).map((color, colorIdx) => (
                              <div key={colorIdx} className="flex items-center gap-2 bg-gray-50 rounded-lg p-2">
                                <input
                                  type="text"
                                  value={color.color_name ?? ''}
                                  onChange={(e) => handleEditItemColorChange(itemIdx, colorIdx, 'color_name', e.target.value)}
                                  placeholder="اسم اللون"
                                  className="flex-1 text-xs px-2 py-1 border border-gray-200 rounded focus:outline-none focus:ring-1 focus:ring-[#FA3145]"
                                />
                                <div className="flex items-center gap-1">
                                  <span className="text-xs text-gray-500">#</span>
                                  <input
                                    type="text"
                                    value={color.color_hex ?? ''}
                                    onChange={(e) => handleEditItemColorChange(itemIdx, colorIdx, 'color_hex', e.target.value)}
                                    placeholder="رمز اللون"
                                    className="w-16 text-xs px-2 py-1 border border-gray-200 rounded focus:outline-none focus:ring-1 focus:ring-[#FA3145] font-mono"
                                  />
                                  {color.color_hex && (
                                    <span className="w-5 h-5 rounded-full border border-gray-300 flex-shrink-0" style={{ backgroundColor: `#${color.color_hex}` }} />
                                  )}
                                </div>
                                <input
                                  type="number"
                                  value={color.quantity ?? ''}
                                  onChange={(e) => handleEditItemColorChange(itemIdx, colorIdx, 'quantity', Number(e.target.value))}
                                  className="w-14 text-xs px-2 py-1 border border-gray-200 rounded focus:outline-none focus:ring-1 focus:ring-[#FA3145]"
                                  min="1"
                                  placeholder="العدد"
                                />
                                <button
                                  onClick={() => handleEditItemRemoveColor(itemIdx, colorIdx)}
                                  className="p-1 hover:bg-red-100 rounded text-red-500 cursor-pointer"
                                  title="حذف اللون"
                                >
                                  <X size={14} />
                                </button>
                              </div>
                            ))}
                            <button
                              onClick={() => handleEditItemAddColor(itemIdx)}
                              className="text-xs text-blue-600 hover:text-blue-800 cursor-pointer"
                            >
                              + إضافة لون
                            </button>
                          </>
                        )}
                      </div>
                    </div>
                    );
                  })}
                </div>
              </div>
            </div>

            <div className="p-6 border-t border-gray-200 flex justify-end gap-3">
              <button onClick={() => setEditingOrder(null)} disabled={isSaving} className="px-6 py-2 border border-gray-300 rounded-lg text-gray-700 hover:bg-gray-50 transition-colors disabled:opacity-50">
                إلغاء
              </button>
              <button onClick={handleSave} disabled={isSaving} className={`px-6 py-2 rounded-lg text-white transition-colors ${isSaving ? 'bg-blue-300 cursor-not-allowed' : 'bg-blue-600 hover:bg-blue-700'}`}>
                {isSaving ? 'جاري الحفظ...' : 'حفظ التغييرات'}
              </button>
            </div>
          </div>
        </div>
      )}

    </div>
  );
}
