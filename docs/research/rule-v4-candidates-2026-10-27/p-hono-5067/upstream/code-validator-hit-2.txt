export interface ValidationRule {
  required?: boolean;
  type?: 'string' | 'number' | 'boolean' | 'object' | 'array';
  minLength?: number;
  maxLength?: number;
  min?: number;
  max?: number;
  pattern?: RegExp;
  custom?: (value: unknown) => boolean | string;
  allowNull?: boolean;
}

export interface ValidationSchema {
  [key: string]: ValidationRule;
}

export interface ValidationResult {
  isValid: boolean;
  errors: Record<string, string>;
  sanitizedData?: Record<string, unknown>;
}

class Validator {
  public validate(data: unknown, schema: ValidationSchema): ValidationResult {
    const errors: Record<string, string> = {};
    const sanitizedData: Record<string, unknown> = {};

    if (typeof data !== 'object' || data === null) {
      return {
        isValid: false,
        errors: { root: 'Data must be an object' }
      };
    }

    const dataObj = data as Record<string, unknown>;

    for (const [field, rule] of Object.entries(schema)) {
      const value = dataObj[field];
      const fieldError = this.validateField(field, value, rule);

      if (fieldError) {
        errors[field] = fieldError;
      } else {
        sanitizedData[field] = this.sanitizeValue(value, rule);
      }
    }

    return {
      isValid: Object.keys(errors).length === 0,
      errors,
      sanitizedData: Object.keys(errors).length === 0 ? sanitizedData : undefined
    };
  }

  private validateField(field: string, value: unknown, rule: ValidationRule): string | null {
    // Check required
    if (rule.required && (value === undefined || value === null || value === '')) {
      return `${field} is required`;
    }

    // Allow null if explicitly allowed and value is null/undefined
    if (rule.allowNull && (value === null || value === undefined)) {
      return null;
    }

    // Skip validation if value is undefined and not required
    if (value === undefined && !rule.required) {
      return null;
    }

    // Type validation
    if (rule.type && !this.validateType(value, rule.type)) {
      return `${field} must be of type ${rule.type}`;
    }

    // String validations
    if (rule.type === 'string' && typeof value === 'string') {
      if (rule.minLength && value.length < rule.minLength) {
        return `${field} must be at least ${rule.minLength} characters long`;
      }
      if (rule.maxLength && value.length > rule.maxLength) {
        return `${field} must be no more than ${rule.maxLength} characters long`;
      }
      if (rule.pattern && !rule.pattern.test(value)) {
        return `${field} format is invalid`;
      }
    }

    // Number validations
    if (rule.type === 'number' && typeof value === 'number') {
      if (rule.min !== undefined && value < rule.min) {
        return `${field} must be at least ${rule.min}`;
      }
      if (rule.max !== undefined && value > rule.max) {
        return `${field} must be no more than ${rule.max}`;
      }
      if (!isFinite(value)) {
        return `${field} must be a finite number`;
      }
    }

    // Array validations
    if (rule.type === 'array' && Array.isArray(value)) {
      if (rule.minLength && value.length < rule.minLength) {
        return `${field} must have at least ${rule.minLength} items`;
      }
      if (rule.maxLength && value.length > rule.maxLength) {
        return `${field} must have no more than ${rule.maxLength} items`;
      }
    }

    // Custom validation
    if (rule.custom) {
      const customResult = rule.custom(value);
      if (typeof customResult === 'string') {
        return customResult;
      }
      if (customResult === false) {
        return `${field} is invalid`;
      }
    }

    return null;
  }

  private validateType(value: unknown, type: string): boolean {
    switch (type) {
      case 'string':
        return typeof value === 'string';
      case 'number':
        return typeof value === 'number' && !isNaN(value);
      case 'boolean':
        return typeof value === 'boolean';
      case 'object':
        return typeof value === 'object' && value !== null && !Array.isArray(value);
      case 'array':
        return Array.isArray(value);
      default:
        return false;
    }
  }

  private sanitizeValue(value: unknown, rule: ValidationRule): unknown {
    if (value === null || value === undefined) {
      return value;
    }

    switch (rule.type) {
      case 'string':
        // Special handling for address fields
        if (rule.pattern && rule.pattern.source.includes('0x[a-fA-F0-9]{64}')) {
          // This is an address field, handle different formats
          let addressStr: string;
          
          if (typeof value === 'string') {
            addressStr = value;
          } else if (typeof value === 'object' && value !== null) {
            if ('data' in value && Array.isArray((value as { data: number[] }).data)) {
              const addressData = (value as { data: number[] }).data;
              addressStr = '0x' + addressData.map((byte: number) => 
                byte.toString(16).padStart(2, '0')
              ).join('');
            } else if ('address' in value && typeof (value as { address: string }).address === 'string') {
              addressStr = (value as { address: string }).address;
            } else if (Array.isArray(value)) {
              addressStr = '0x' + value.map((byte: number) => 
                byte.toString(16).padStart(2, '0')
              ).join('');
            } else {
              addressStr = String(value);
            }
          } else {
            addressStr = String(value);
          }
          
          return addressStr.trim();
        }
        
        // Regular string handling
        if (typeof value === 'string') {
          return value.trim();
        }
        return String(value).trim();
      case 'number':
        if (typeof value === 'number') {
          return value;
        }
        const num = parseFloat(String(value));
        return isNaN(num) ? value : num;
      case 'boolean':
        if (typeof value === 'boolean') {
          return value;
        }
        if (typeof value === 'string') {
          return value.toLowerCase() === 'true';
        }
        return Boolean(value);
      default:
        return value;
    }
  }
}

// Common validation schemas
export const schemas = {
  address: {
    required: true,
    type: 'string' as const,
    pattern: /^0x[a-fA-F0-9]{64}$/,
    minLength: 66,
    maxLength: 66,
    custom: (value: unknown) => {
      // Handle different address formats from wallets
      let addressStr: string;
      
      if (typeof value === 'string') {
        addressStr = value;
      } else if (typeof value === 'object' && value !== null) {
        // Handle Uint8Array or object with address property
        if ('data' in value && Array.isArray((value as { data: number[] }).data)) {
          const addressData = (value as { data: number[] }).data;
          addressStr = '0x' + addressData.map((byte: number) => 
            byte.toString(16).padStart(2, '0')
          ).join('');
        } else if ('address' in value && typeof (value as { address: string }).address === 'string') {
          addressStr = (value as { address: string }).address;
        } else if (Array.isArray(value)) {
          addressStr = '0x' + value.map((byte: number) => 
            byte.toString(16).padStart(2, '0')
          ).join('');
        } else {
          return 'Invalid address format';
        }
      } else {
        return 'Address must be a string or valid address object';
      }
      
      // Validate the final address string
      if (!/^0x[a-fA-F0-9]{64}$/.test(addressStr)) {
        return 'Address must be a valid 64-character hex string starting with 0x';
      }
      
      return true;
    }
  },

  amount: {
    required: true,
    type: 'number' as const,
    min: 0.000001,
    max: 1000000,
    custom: (value: unknown) => {
      if (typeof value === 'number' && isFinite(value)) {
        return true;
      }
      return 'Amount must be a valid finite number';
    }
  },

  token: {
    required: true,
    type: 'string' as const,
    pattern: /^(USDC|USDT|APT)$/,
    custom: (value: unknown) => {
      const validTokens = ['USDC', 'USDT', 'APT'];
      return typeof value === 'string' && validTokens.includes(value);
    }
  },

  interestRate: {
    required: true,
    type: 'number' as const,
    min: 0.1,
    max: 50,
    custom: (value: unknown) => {
      if (typeof value === 'number' && value > 0 && value <= 50) {
        return true;
      }
      return 'Interest rate must be between 0.1% and 50%';
    }
  },

  duration: {
    required: false,
    type: 'number' as const,
    min: 1,
    max: 365,
    custom: (value: unknown) => {
      if (value === undefined) return true;
      if (typeof value === 'number' && Number.isInteger(value) && value >= 1 && value <= 365) {
        return true;
      }
      return 'Duration must be an integer between 1 and 365 days';
    }
  },

  loanId: {
    required: true,
    type: 'string' as const,
    minLength: 8,
    maxLength: 64,
    pattern: /^[a-zA-Z0-9_-]+$/
  },

  predictionData: {
    required: true,
    type: 'array' as const,
    minLength: 3,
    maxLength: 3,
    custom: (value: unknown) => {
      if (!Array.isArray(value) || value.length !== 3) {
        return 'Prediction data must be an array of exactly 3 numbers';
      }
      
      const [gasPrice, txVolume, timestamp] = value;
      
      if (typeof gasPrice !== 'number' || gasPrice < 0.001 || gasPrice > 10) {
        return 'Gas price must be between 0.001 and 10';
      }
      
      if (typeof txVolume !== 'number' || txVolume < 1 || txVolume > 1000000) {
        return 'Transaction volume must be between 1 and 1,000,000';
      }
      
      if (typeof timestamp !== 'number' || timestamp < 0 || timestamp > 24 * 60) {
        return 'Timestamp must be between 0 and 1440 minutes';
      }
      
      return true;
    }
  }
};

// Predefined validation schemas for API endpoints
export const apiSchemas = {
  transfer: {
    senderAddress: schemas.address,
    recipientAddress: schemas.address,
    amount: schemas.amount,
    token: schemas.token,
    signedTransaction: {
      required: false,
      type: 'object' as const,
      allowNull: true
    }
  },

  lendingOffer: {
    lenderAddress: schemas.address,
    amount: schemas.amount,
    interestRate: schemas.interestRate,
    token: schemas.token,
    duration: schemas.duration,
    signedTransaction: {
      required: false,
      type: 'object' as const,
      allowNull: true
    }
  },

  lendingAccept: {
    borrowerAddress: schemas.address,
    lenderAddress: schemas.address,
    loanId: schemas.loanId,
    signedTransaction: {
      required: false,
      type: 'object' as const,
      allowNull: true
    }
  },

  prediction: {
    type: {
      required: true,
      type: 'string' as const,
      pattern: /^(fee|fraud)$/,
      custom: (value: unknown) => {
        return typeof value === 'string' && ['fee', 'fraud'].includes(value);
      }
    },
    data: schemas.predictionData,
    options: {
      required: false,
      type: 'object' as const,
      allowNull: true
    }
  }
};

// Security validation functions
export function sanitizeInput(input: string): string {
  return input
    .replace(/[<>\"'&]/g, '') // Remove potentially dangerous characters
    .trim()
    .slice(0, 1000); // Limit length
}

export function validateIPAddress(ip: string): boolean {
  const ipv4Regex = /^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/;
  const ipv6Regex = /^(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}$/;
  return ipv4Regex.test(ip) || ipv6Regex.test(ip);
}

export function validateUserAgent(userAgent: string): boolean {
  // Basic user agent validation - should contain browser/version info
  const userAgentRegex = /^[a-zA-Z0-9\s\.\-_\(\)\/;:,]+$/;
  return userAgentRegex.test(userAgent) && userAgent.length < 512;
}

export function validateContentType(contentType: string): boolean {
  const allowedTypes = [
    'application/json',
    'application/x-www-form-urlencoded',
    'multipart/form-data'
  ];
  return allowedTypes.some(type => contentType.startsWith(type));
}

export const validator = new Validator();
