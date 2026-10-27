
export class Validator {
    /**
     * Valida se o valor não é nulo ou indefinido.
     */
    static required(value: any, fieldName: string): string | null {
        if (value === undefined || value === null || value === '') {
            return `O campo '${fieldName}' é obrigatório.`;
        }
        return null;
    }

    /**
     * Valida se o valor é uma string e se tem o tamanho mínimo/máximo.
     */
    static isString(value: any, fieldName: string, min?: number, max?: number): string | null {
        if (typeof value !== 'string') {
            return `O campo '${fieldName}' deve ser um texto.`;
        }
        if (min !== undefined && value.length < min) {
            return `O campo '${fieldName}' deve ter no mínimo ${min} caracteres.`;
        }
        if (max !== undefined && value.length > max) {
            return `O campo '${fieldName}' deve ter no máximo ${max} caracteres.`;
        }
        return null;
    }

    /**
     * Valida se o formato do e-mail é válido.
     */
    static isEmail(value: any, fieldName: string = 'email'): string | null {
        const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
        if (typeof value !== 'string' || !emailRegex.test(value)) {
            return `O campo '${fieldName}' deve ser um e-mail válido.`;
        }
        return null;
    }

    /**
     * Valida se o valor é um número (ou string numérica) e tenta convertê-lo.
     * Retorna null se for válido, ou mensagem de erro.
     * NOTE: Essa função apenas checa, para obter o valor convertido use parseNumber.
     */
    static isNumber(value: any, fieldName: string): string | null {
        if (value === undefined || value === null || value === '') return null; // Use required() se for obrigatório

        const num = Number(value);
        if (isNaN(num)) {
            return `O campo '${fieldName}' deve ser um número válido.`;
        }
        return null;
    }

    /**
     * Converte string para número de forma segura.
     * Útil para lidar com inputs de multipart/form-data.
     */
    static parseNumber(value: any): number | undefined {
        if (value === undefined || value === null || value === '') return undefined;
        const num = Number(value);
        return isNaN(num) ? undefined : num;
    }
}
