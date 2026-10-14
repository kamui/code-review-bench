import { ArrowLeft, LayoutTemplate, Save, X, Plus, Trash2, Pencil, Tag, List, Type, ListOrdered, Ruler, Hash, AlertCircle, FileText } from 'lucide-react';
import { useForm } from 'react-hook-form';
import { zodResolver } from '@hookform/resolvers/zod';
import * as z from 'zod';
import { useNavigate, useParams } from 'react-router-dom';
import toast from 'react-hot-toast';
import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Checkbox } from '@/components/ui/checkbox';
import {
  getItemConfigurationById,
  updateItemConfiguration,
  createMultipleItemConfigurations
} from '@/services/itemService';
import type { ItemConfigData } from '@/services/itemService';

// Zod schema for individual field
const fieldSchema = z.object({
  label: z.string().min(1, 'Label is required'),
  description: z.string().optional(),
  controlType: z.enum(['Textbox', 'Dropdown', 'Textarea'], {
    required_error: 'Control type is required',
  }),
  collectionName: z.string().nullable().optional(),
  dataType: z.enum(['text', 'number', 'unit']).nullable().optional(),
  order: z.number().min(1),
  maxLength: z.number().min(0),
  measurementType: z.string().nullable().optional(),
  isMandatory: z.boolean().optional(),
}).superRefine((data, ctx) => {
  if (data.controlType === 'Dropdown' && !data.collectionName) {
    ctx.addIssue({
      path: ['collectionName'],
      message: 'Collection name is required for Dropdown',
      code: z.ZodIssueCode.custom,
    });
  }
  if (data.controlType === 'Textbox' && !data.dataType) {
    ctx.addIssue({
      path: ['dataType'],
      message: 'Data type is required for Textbox',
      code: z.ZodIssueCode.custom,
    });
  }
});

type FieldFormValues = z.infer<typeof fieldSchema>;

interface UnitOption {
  id: string;
  name: string;
}

interface CollectionOption {
  id: string;
  display_name: string;
}

// Define the type for the item configuration data
interface ItemConfigDataType {
  id?: string;
  name: string;
  description?: string;
  control_type: 'Textbox' | 'Dropdown' | 'Textarea';
  collection_id?: string | null | { toString: () => string };
  data_type?: 'text' | 'number' | 'unit';
  sequence: number;
  max_length?: number;
  item_unit_id?: string | null | { toString: () => string };
  is_mandatory: boolean;
}

const ItemConfigForm = () => {
  const { id } = useParams();
  const navigate = useNavigate();
  const isEditMode = Boolean(id);
  const user = localStorage.getItem('userData');
  const userData = user ? JSON.parse(user) : null;
  const [units, setUnits] = useState<UnitOption[]>([]);
  const [collections, setCollections] = useState<CollectionOption[]>([]);
  const [addedFields, setAddedFields] = useState<FieldFormValues[]>([]);
  const [showForm, setShowForm] = useState(true);
  const [loading, setLoading] = useState(false);

  const {
    register,
    handleSubmit,
    formState: { errors, isSubmitting },
    setValue,
    watch,
    reset,
    trigger,
  } = useForm<FieldFormValues>({
    resolver: zodResolver(fieldSchema),
    defaultValues: {
      label: '',
      description: '',
      controlType: undefined,
      collectionName: undefined,
      dataType: undefined,
      order: 0,
      maxLength: 0,
      measurementType: undefined,
      isMandatory: false,
    },
  });

  const watchedFields = watch();

  useEffect(() => {
    if (isEditMode) {
      getFieldDetails();
      setShowForm(true);
    }
  }, [id]);

  useEffect(() => {
    const fetchUnitsAndCollections = async () => {
      try {
        // In a real implementation, you would fetch these from the backend
        // For now, using mock data
        setUnits([
          { id: '1', name: 'Pieces' },
          { id: '2', name: 'Meters' },
          { id: '3', name: 'Kilograms' },
          { id: '4', name: 'Liters' }
        ]);
        
        setCollections([
          { id: '1', display_name: 'Colors' },
          { id: '2', display_name: 'Sizes' },
          { id: '3', display_name: 'Brands' }
        ]);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        console.error('Fetch Units/Collections Error =>', message);
        toast.error('Failed to fetch units/collections: ' + message);
      }
    };

    fetchUnitsAndCollections();
  }, []);

  const getFieldDetails = async () => {
    try {
      if (!id) return;
      
      setLoading(true);
      const response = await getItemConfigurationById(id);
      
      const data = response as ItemConfigDataType;
      if (data) {
        // Handle collection_id - it might be an ObjectId or string
        const collectionId = data.collection_id 
          ? (typeof data.collection_id === 'object' && data.collection_id !== null && 'toString' in data.collection_id
              ? data.collection_id.toString() 
              : String(data.collection_id))
          : undefined;
        
        // Handle item_unit_id - it might be an ObjectId or string
        const itemUnitId = data.item_unit_id
          ? (typeof data.item_unit_id === 'object' && data.item_unit_id !== null && 'toString' in data.item_unit_id
              ? data.item_unit_id.toString()
              : String(data.item_unit_id))
          : undefined;
        
        reset({
          label: data.name || '',
          description: data.description || '',
          controlType: data.control_type as 'Textbox' | 'Dropdown' | 'Textarea',
          collectionName: collectionId,
          dataType: data.data_type as 'text' | 'number' | 'unit' | undefined,
          order: data.sequence || 0,
          maxLength: data.max_length || 0,
          measurementType: itemUnitId,
          isMandatory: data.is_mandatory || false,
        });
      }
    } catch (error) {
      const message =
        (error as { response?: { data?: { message?: string } } })?.response?.data?.message ||
        (error instanceof Error ? error.message : 'Unknown error');
      console.error('Fetch error =>', error);
      const errorMessage = message || 'Unknown error';
      toast.error('Failed to fetch field data: ' + errorMessage);
    } finally {
      setLoading(false);
    }
  };

  const handleAddField = async (data: FieldFormValues) => {
    const isValid = await trigger();
    if (!isValid) return;

    const newField = { ...data };

    // Check duplicate display order in preview list
    if (addedFields.some(field => field.order === newField.order)) {
      toast.error(`Display order ${newField.order} already exists in the preview list. Please choose a different number.`);
      return;
    }

    // Check duplicate display order in database
    try {
      // In a real implementation, you would check if the sequence already exists
      // For now, we'll skip this check
    } catch (error: any) {
      console.error('Error checking existing sequences:', error);
      toast.error('Error checking existing sequences: ' + (error.message || 'Unknown error'));
      return;
    }

    setAddedFields(prev => [...prev, newField]);
    handleResetForm();
  };

  const createFieldConfigs = async () => {
    try {
      if (!userData?.id) {
        toast.error('User data not found. Please login again.');
        return;
      }

      if (addedFields.length === 0) {
        toast.error('Please add at least one field before saving.');
        return;
      }

      // Convert frontend data to backend format
      const payload = addedFields.map((field) => ({
        name: field.label,
        description: field.description || '',
        control_type: field.controlType,
        collection_id: field.controlType === 'Dropdown' ? field.collectionName : undefined,
        data_type: field.controlType === 'Textbox' && field.dataType !== null ? field.dataType : undefined,
        sequence: field.order,
        max_length: field.controlType === 'Textbox' && field.dataType === 'text' ? field.maxLength : 0,
        item_unit_id: field.dataType === 'unit' ? field.measurementType : undefined,
        is_mandatory: field.isMandatory || false,
      }));

      setLoading(true);
      const response = await createMultipleItemConfigurations(payload);
      
      if (response) {
        toast.success('Item fields created successfully!');
        setAddedFields([]);
        handleResetForm();
        navigate('/dashboard/itemConfigurator');
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      console.error('Error submitting form:', message);
      toast.error('Failed to create item fields: ' + message);
    } finally {
      setLoading(false);
    }
  };

  const updateFieldConfig = async (data: FieldFormValues) => {
    try {
      if (!id) {
        toast.error('Invalid field ID.');
        return;
      }

      // Convert frontend data to backend format
      const payload: Partial<ItemConfigData> = {
        name: data.label,
        description: data.description || '',
        control_type: data.controlType,
        collection_id: data.controlType === 'Dropdown' && data.collectionName ? data.collectionName : undefined,
        data_type: data.controlType === 'Textbox' && data.dataType !== null ? data.dataType : undefined,
        sequence: data.order,
        max_length: data.controlType === 'Textbox' && data.dataType === 'text' ? (data.maxLength || 0) : 0,
        item_unit_id: data.dataType === 'unit' && data.measurementType ? data.measurementType : undefined,
        is_mandatory: data.isMandatory || false,
      };
      
      // Remove undefined values to avoid validation issues
      Object.keys(payload).forEach((key) => {
        if (payload[key as keyof ItemConfigData] === undefined) {
          delete payload[key as keyof ItemConfigData];
        }
      });

      setLoading(true);
      const response = await updateItemConfiguration(id, payload);
      
      if (response) {
        toast.success('Item field updated successfully!');
        handleResetForm();
        navigate('/dashboard/itemConfigurator');
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Unknown error';
      console.error('Error updating field:', message);
      toast.error('Failed to update item field: ' + message);
    } finally {
      setLoading(false);
    }
  };

  const handleResetForm = () => {
    reset({
      label: '',
      description: '',
      controlType: undefined,
      collectionName: undefined,
      dataType: undefined,
      order: 0,
      maxLength: 0,
      measurementType: undefined,
      isMandatory: false,
    });
  };

  const removeAddedField = (index: number) => {
    setAddedFields((prev) => prev.filter((_, i) => i !== index));
  };

  const onSubmit = async (data: FieldFormValues) => {
    console.log("data =>", data);
    if (isEditMode) {
      await updateFieldConfig(data);
    } else {
      await handleAddField(data);
    }
  };

  console.log("Form errors =>", errors);
  
  if (loading && isEditMode) {
    return (
      <div className="p-6 bg-gray-50 min-h-screen flex items-center justify-center">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-600 mx-auto"></div>
          <p className="mt-4 text-gray-600">Loading field data...</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-6 bg-gray-50 min-h-screen">
      <div className="max-w-6xl mx-auto space-y-8">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-4">
            <Button
              variant="ghost"
              size="icon"
              onClick={() => navigate('/dashboard/itemConfigurator')}
              className="hover:bg-blue-100 transition-colors duration-200 rounded-full"
            >
              <ArrowLeft className="h-5 w-5 text-gray-700" />
            </Button>
            <div className="flex items-center space-x-3">
              <div className="p-2 rounded-lg bg-blue-100">
                <LayoutTemplate className="h-6 w-6 text-blue-600" />
              </div>
              <div>
                <h1 className="text-3xl font-bold text-gray-900">
                  {isEditMode ? 'Update Item Field' : 'Add New Fields'}
                </h1>
                <p className="text-gray-600">
                  {isEditMode ? 'Edit your item field configuration' : 'Create multiple item fields at once'}
                </p>
              </div>
            </div>
          </div>
        </div>

        {(isEditMode || showForm) && (
          <Card className="shadow-md border-0 overflow-hidden">
            <CardHeader>
              <div className="flex items-center justify-between">
                <div>
                  <CardTitle className="text-xl text-blue-800">
                    {isEditMode ? 'Update Field Configuration' : 'Add Field Configuration'}
                  </CardTitle>
                  <CardDescription className="text-blue-600">
                    {isEditMode ? 'Edit your field configuration' : 'Define how your field should appear and behave'}
                  </CardDescription>
                </div>
              </div>
            </CardHeader>
            <CardContent className="pt-2">
              <form onSubmit={handleSubmit(onSubmit)} className="space-y-6">
                <div className="space-y-5">
                  <div className="space-y-2 group">
                    <Label
                      htmlFor="label"
                      className={`${errors.label ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                    >
                      <Tag className="h-4 w-4" /> Field Label <span className="text-red-500">*</span>
                    </Label>
                    <Input
                      id="label"
                      placeholder="e.g. Email Address"
                      {...register('label')}
                      className={`${errors.label
                        ? 'border-red-300 focus:border-red-500 focus:ring-red-200'
                        : 'border-gray-200 focus:border-blue-500 focus:ring-blue-200'
                      } pl-3 pr-3 py-2 rounded-md shadow-sm focus:ring-4 transition-all duration-200 ${
                        watchedFields.label ? 'border-blue-300' : ''
                      }`}
                    />
                    {errors.label && (
                      <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                        <AlertCircle className="h-3 w-3" />
                        {errors.label.message}
                      </p>
                    )}
                  </div>

                  <div className="space-y-2 group">
                    <Label
                      htmlFor="description"
                      className={`${errors.description ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                    >
                      <FileText className="h-4 w-4" /> Description
                    </Label>
                    <Input
                      id="description"
                      placeholder="e.g. Enter a brief description of the field"
                      {...register('description')}
                      className={`${errors.description
                        ? 'border-red-300 focus:border-red-500 focus:ring-red-200'
                        : 'border-gray-200 focus:border-blue-500 focus:ring-blue-200'
                      } pl-3 pr-3 py-2 rounded-md shadow-sm focus:ring-4 transition-all duration-200 ${
                        watchedFields.description ? 'border-blue-300' : ''
                      }`}
                    />
                    {errors.description && (
                      <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                        <AlertCircle className="h-3 w-3" />
                        {errors.description.message}
                      </p>
                    )}
                  </div>

                  <div className="space-y-2 group">
                    <Label
                      htmlFor="isMandatory"
                      className={`${errors.isMandatory ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                    >
                      <Tag className="h-4 w-4" /> Mandatory Field
                    </Label>
                    <div className="flex items-center space-x-2">
                      <Checkbox
                        id="isMandatory"
                        {...register('isMandatory')}
                        checked={watchedFields.isMandatory}
                        onCheckedChange={(checked) => setValue('isMandatory', checked === true)}
                      />
                      <Label htmlFor="isMandatory" className="text-sm text-gray-600">
                        Mark this field as mandatory
                      </Label>
                    </div>
                    {errors.isMandatory && (
                      <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                        <AlertCircle className="h-3 w-3" />
                        {errors.isMandatory.message}
                      </p>
                    )}
                  </div>

                  <div className="space-y-2 group">
                    <Label
                      className={`${errors.controlType ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                    >
                      <List className="h-4 w-4" /> Control Type <span className="text-red-500">*</span>
                    </Label>
                    <Select
                      value={watchedFields.controlType || ''}
                      onValueChange={(value) => {
                        setValue('controlType', value as FieldFormValues['controlType']);
                        setValue('collectionName', undefined);
                        setValue('dataType', undefined);
                        setValue('measurementType', undefined);
                      }}
                    >
                      <SelectTrigger
                        className={`${errors.controlType
                          ? 'border-red-300 focus:border-red-500 focus:ring-red-200'
                          : 'border-gray-200 focus:border-blue-500 focus:ring-blue-200'
                        } pl-3 pr-3 py-2 rounded-md shadow-sm focus:ring-4 transition-all duration-200 w-full ${
                          watchedFields.controlType ? 'border-blue-300' : ''
                        }`}
                      >
                        <SelectValue placeholder="Select Control Type" />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="Textbox">Textbox</SelectItem>
                        {/* <SelectItem value="Dropdown">Dropdown</SelectItem> */}
                        <SelectItem value="Textarea">Textarea</SelectItem>
                      </SelectContent>
                    </Select>
                    {errors.controlType && (
                      <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                        <AlertCircle className="h-3 w-3" />
                        {errors.controlType.message}
                      </p>
                    )}
                  </div>

                  {watchedFields.controlType === 'Dropdown' && (
                    <div className="space-y-2 group animate-fadeIn">
                      <Label
                        className={`${errors.collectionName ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                      >
                        <ListOrdered className="h-4 w-4" /> Collection Name *
                      </Label>
                      <Select
                        value={watchedFields.collectionName || ''}
                        onValueChange={(value) => setValue('collectionName', value as FieldFormValues['collectionName'])}
                      >
                        <SelectTrigger
                          className={`${errors.collectionName
                            ? 'border-red-300 focus:border-red-500 focus:ring-red-200'
                            : 'border-gray-200 focus:border-blue-500 focus:ring-blue-200'
                          } pl-3 pr-3 py-2 rounded-md shadow-sm focus:ring-4 transition-all duration-200 w-full ${
                            watchedFields.collectionName ? 'border-blue-300' : ''
                          }`}
                        >
                          <SelectValue placeholder="Select Collection Name" />
                        </SelectTrigger>
                        <SelectContent>
                          {collections.map((collection) => (
                            <SelectItem key={collection.id} value={collection.id}>
                              {collection.display_name}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      {errors.collectionName && (
                        <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                          <AlertCircle className="h-3 w-3" />
                          {errors.collectionName.message}
                        </p>
                      )}
                    </div>
                  )}

                  {watchedFields.controlType === 'Textbox' && (
                    <div className="space-y-2 group animate-fadeIn">
                      <Label
                        className={`${errors.dataType ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                      >
                        <Type className="h-4 w-4" /> Data Type <span className="text-red-500">*</span>
                      </Label>
                      <Select
                        value={watchedFields.dataType || ''}
                        onValueChange={(value) => {
                          setValue('dataType', value as FieldFormValues['dataType']);
                          setValue('measurementType', undefined);
                        }}
                      >
                        <SelectTrigger
                          className={`${errors.dataType
                            ? 'border-red-300 focus:border-red-500 focus:ring-red-200'
                            : 'border-gray-200 focus:border-blue-500 focus:ring-blue-200'
                          } pl-3 pr-3 py-2 rounded-md shadow-sm focus:ring-4 transition-all duration-200 w-full ${
                            watchedFields.dataType ? 'border-blue-300' : ''
                          }`}
                        >
                          <SelectValue placeholder="Select Data Type" />
                        </SelectTrigger>
                        <SelectContent>
                          <SelectItem value="text">Text</SelectItem>
                          <SelectItem value="number">Number</SelectItem>
                          <SelectItem value="unit">Unit</SelectItem>
                        </SelectContent>
                      </Select>
                      {errors.dataType && (
                        <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                          <AlertCircle className="h-3 w-3" />
                          {errors.dataType.message}
                        </p>
                      )}
                    </div>
                  )}

                  {watchedFields.dataType === 'unit' && (
                    <div className="space-y-2 group animate-fadeIn">
                      <Label
                        className={`${errors.measurementType ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                      >
                        <Ruler className="h-4 w-4" /> Measurement Type
                      </Label>
                      <Select
                        value={watchedFields.measurementType || ''}
                        onValueChange={(value) => setValue('measurementType', value as FieldFormValues['measurementType'])}
                      >
                        <SelectTrigger
                          className={`${errors.measurementType
                            ? 'border-red-300 focus:border-red-500 focus:ring-red-200'
                            : 'border-gray-200 focus:border-blue-500 focus:ring-blue-200'
                          } pl-3 pr-3 py-2 rounded-md shadow-sm focus:ring-4 transition-all duration-200 w-full ${
                            watchedFields.measurementType ? 'border-blue-300' : ''
                          }`}
                        >
                          <SelectValue placeholder="Select Measurement Type" />
                        </SelectTrigger>
                        <SelectContent>
                          {units.map((unit) => (
                            <SelectItem key={unit.id} value={unit.id}>
                              {unit.name}
                            </SelectItem>
                          ))}
                        </SelectContent>
                      </Select>
                      {errors.measurementType && (
                        <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                          <AlertCircle className="h-3 w-3" />
                          {errors.measurementType.message}
                        </p>
                      )}
                    </div>
                  )}

                  <div className="grid grid-cols-2 gap-4">
                    <div className="space-y-2 group">
                      <Label
                        htmlFor="order"
                        className={`${errors.order ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                      >
                        <ListOrdered className="h-4 w-4" /> Display Order
                      </Label>
                      <Input
                        id="order"
                        type="number"
                        {...register('order', { valueAsNumber: true })}
                        className={`${errors.order
                          ? 'border-red-300 focus:border-red-500 focus:ring-red-200'
                          : 'border-gray-200 focus:border-blue-500 focus:ring-blue-200'
                        } pl-3 pr-3 py-2 rounded-md shadow-sm focus:ring-4 transition-all duration-200 ${
                          watchedFields.order ? 'border-blue-300' : ''
                        }`}
                      />
                      {errors.order && (
                        <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                          <AlertCircle className="h-3 w-3" />
                          {errors.order.message}
                        </p>
                      )}
                    </div>

                    <div className="space-y-2 group">
                      <Label
                        htmlFor="maxLength"
                        className={`${errors.maxLength ? 'text-red-500' : 'text-gray-700'} group-hover:text-blue-700 transition-colors duration-200 flex items-center gap-1 font-medium`}
                      >
                        <Hash className="h-4 w-4" /> Max Length
                        {(watchedFields.controlType === 'Dropdown' || watchedFields.dataType !== 'text') && (
                          <span className="text-xs text-gray-500 ml-1">(Not applicable)</span>
                        )}
                      </Label>
                      <Input
                        id="maxLength"
                        type="number"
                        disabled={watchedFields.controlType === 'Dropdown' || watchedFields.dataType !== 'text'}
                        {...register('maxLength', { valueAsNumber: true })}
                        className={`${errors.maxLength && watchedFields.controlType !== 'Dropdown' && watchedFields.dataType === 'text'
                          ? 'border-red-300 focus:border-red-500 focus:ring-red-200'
                          : 'border-gray-200 focus:border-blue-500 focus:ring-blue-200'
                        } pl-3 pr-3 py-2 rounded-md shadow-sm focus:ring-4 transition-all duration-200 ${
                          watchedFields.controlType === 'Dropdown' || watchedFields.dataType !== 'text'
                            ? 'bg-gray-100 text-gray-400 cursor-not-allowed'
                            : watchedFields.maxLength
                            ? 'border-blue-300'
                            : ''
                        }`}
                        placeholder={
                          watchedFields.controlType === 'Dropdown' || watchedFields.dataType !== 'text'
                            ? 'N/A'
                            : 'Enter max length'
                        }
                      />
                      {errors.maxLength && watchedFields.controlType !== 'Dropdown' && watchedFields.dataType === 'text' && (
                        <p className="text-sm text-red-500 flex items-center gap-1 mt-1">
                          <AlertCircle className="h-3 w-3" />
                          {errors.maxLength.message}
                        </p>
                      )}
                    </div>
                  </div>
                </div>

                <div className="flex gap-4 pt-4 justify-end">
                  <Button
                    type="button"
                    variant="outline"
                    onClick={() => navigate('/dashboard/itemConfigurator')}
                    className="flex items-center gap-2 border-gray-300 text-gray-700 hover:bg-gray-100 transition-colors duration-200"
                  >
                    <X className="h-4 w-4" />
                    Cancel
                  </Button>

                  <Button
                    type="submit"
                    disabled={isSubmitting || loading}
                    className="bg-blue-600 hover:bg-blue-700 flex items-center gap-2 text-white transition-all duration-300 transform hover:scale-105 shadow-md hover:shadow-lg"
                  >
                    <Plus className="h-4 w-4" />
                    {isSubmitting || loading ? (isEditMode ? 'Updating...' : 'Adding...') : isEditMode ? 'Update Field' : 'Add Field'}
                  </Button>
                </div>
              </form>
            </CardContent>
          </Card>
        )}

        {!isEditMode && addedFields.length > 0 && (
          <Card className="shadow-md border-0 overflow-hidden">
            <CardHeader>
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div>
                    <CardTitle className="text-xl font-semibold text-green-800">Added Fields Preview</CardTitle>
                    <CardDescription className="text-green-600 mt-1">
                      Review and manage your configured fields before saving
                    </CardDescription>
                  </div>
                </div>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setShowForm(!showForm)}
                  className="text-gray-600 hover:bg-gray-100"
                >
                  {showForm ? (
                    <>
                      <X className="h-4 w-4 mr-2" />
                      Hide Form
                    </>
                  ) : (
                    <>
                      <Plus className="h-4 w-4 mr-2" />
                      Show Form
                    </>
                  )}
                </Button>
              </div>
            </CardHeader>
            <CardContent className="p-6">
              <div className="space-y-6 max-h-[250px] overflow-y-auto">
                {addedFields.map((field, index) => (
                  <div
                    key={index}
                    className="border rounded-lg p-5 bg-green-50 relative"
                    role="region"
                    aria-label={`Field ${field.label}`}
                  >
                    <div className="flex items-start justify-between gap-4">
                      <div className="flex-1">
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
                          <div className="space-y-1">
                            <Label className="text-sm font-semibold text-gray-700">Field Label</Label>
                            <p className="text-sm font-bold text-gray-900">{field.label}</p>
                          </div>
                          <div className="space-y-1">
                            <Label className="text-sm font-semibold text-gray-700">Description</Label>
                            <p className="text-sm font-bold text-gray-900">{field.description || 'None'}</p>
                          </div>
                          <div className="space-y-1">
                            <Label className="text-sm font-semibold text-gray-700">Mandatory</Label>
                            <p className="text-sm font-bold text-gray-900">{field.isMandatory ? 'Yes' : 'No'}</p>
                          </div>
                          <div className="space-y-1">
                            <Label className="text-sm font-semibold text-gray-700">Control Type</Label>
                            <p className="text-sm font-bold text-gray-900">{field.controlType}</p>
                          </div>
                          {field.controlType === 'Dropdown' && field.collectionName && (
                            <div className="space-y-1">
                              <Label className="text-sm font-semibold text-gray-700">Collection</Label>
                              <p className="text-sm font-bold text-gray-900">
                                {collections.find((collection) => collection.id === field.collectionName)?.display_name || 'Unknown'}
                              </p>
                            </div>
                          )}
                          {field.controlType === 'Textbox' && field.dataType && (
                            <div className="space-y-1">
                              <Label className="text-sm font-semibold text-gray-700">Data Type</Label>
                              <p className="text-sm font-bold text-gray-900 capitalize">{field.dataType}</p>
                            </div>
                          )}
                          {field.measurementType && field.dataType === 'unit' && (
                            <div className="space-y-1">
                              <Label className="text-sm font-semibold text-gray-700">Measurement Type</Label>
                              <p className="text-sm font-bold text-gray-900 capitalize">
                                {units.find((unit) => unit.id === field.measurementType)?.name || 'Unknown'}
                              </p>
                            </div>
                          )}
                          <div className="space-y-1">
                            <Label className="text-sm font-semibold text-gray-700">Display Order</Label>
                            <p className="text-sm font-bold text-gray-900">{field.order}</p>
                          </div>
                          {field.controlType === 'Textbox' && field.dataType === 'text' && (
                            <div className="space-y-1">
                              <Label className="text-sm font-semibold text-gray-700">Max Length</Label>
                              <p className="text-sm font-bold text-gray-900">{field.maxLength}</p>
                            </div>
                          )}
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => {
                            reset(field);
                            setShowForm(true);
                            removeAddedField(index);
                          }}
                          className="text-blue-600 hover:text-blue-700 hover:bg-blue-50"
                          aria-label={`Edit field ${field.label}`}
                        >
                          <Pencil className="h-4 w-4" />
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          onClick={() => removeAddedField(index)}
                          className="text-red-600 hover:text-red-700 hover:bg-red-50"
                          aria-label={`Remove field ${field.label}`}
                        >
                          <Trash2 className="h-4 w-4" />
                        </Button>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
              <div className="flex justify-between items-center pt-4">
                <div className="text-sm text-gray-600">
                  Total Fields: <span className="font-semibold">{addedFields.length}</span>
                </div>
                <Button
                  onClick={createFieldConfigs}
                  disabled={isSubmitting || loading}
                  className="bg-green-600 hover:bg-green-700 flex items-center gap-2 px-6 py-2"
                  aria-label="Save all configured fields"
                >
                  <Save className="h-5 w-5" />
                  {isSubmitting || loading ? 'Saving...' : 'Save All Fields'}
                </Button>
              </div>
            </CardContent>
          </Card>
        )}
      </div>
    </div>
  );
};

export default ItemConfigForm;