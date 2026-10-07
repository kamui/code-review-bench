import * as React from 'react';
import { Field } from '@base-ui/react/field';
import { Input } from '@base-ui/react/input';

// Type observations only. The compiler reports an error on each line whose value the
// `value` prop does not accept. Lines without an error are accepted by the declared type.

type ControlValue = NonNullable<Field.Control.Props['value']>;
type InputValue = NonNullable<Input.Props['value']>;

export const acceptsString: ControlValue = 'a,b';
export const acceptsNumber: ControlValue = 5;
export const acceptsStringArray: ControlValue = ['a', 'b'];
export const acceptsNumberArray: ControlValue = [1, 2];
export const acceptsPlainObject: ControlValue = { id: 1 };
export const acceptsNull: Field.Control.Props['value'] = null;
export const inputAcceptsStringArray: InputValue = ['a', 'b'];
export const inputAcceptsPlainObject: InputValue = { id: 1 };

export const elements = (
  <React.Fragment>
    <Field.Control value={['a', 'b']} />
    <Field.Control value={{ id: 1 }} />
  </React.Fragment>
);
