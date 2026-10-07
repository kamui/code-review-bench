import React from "react";

import { Field } from "@base-ui-components/react/field";
import { Form as BaseForm } from "@base-ui-components/react/form";
import { mask } from "@/lib";
import { styled } from "@linaria/react";
import LongPressButton from "./long-press-button";
import { randomIPv4 } from "@/helpers";
import dedent from "dedent";

const SearchWrapper = styled.search`
  align-self: stretch;
`;

const FieldRoot = styled(Field.Root)`
  width: 100%;
`;

const Form = styled(BaseForm)`
  display: flex;

  height: 3.625rem;
  margin-inline: auto;
  max-width: 34.625rem;
`;

const Input = styled(Field.Control)`
  height: 100%;
  padding: 1rem 1.5rem;
  padding-inline-end: 0.25rem;
  width: 100%;

  background-color: white;
  border: none;
  border-top-left-radius: 1rem;
  border-bottom-left-radius: 1rem;
  color: var(--gray);
  font-size: 1.125rem;
  font-weight: var(--fw);

  &::placeholder {
    color: var(--gray-light);
  }
`;

const ErrorMessage = styled(Field.Error)`
  color: crimson;
  font-size: 0.75rem;
  padding-inline-start: 1.5rem;

  display: flex;
  align-items: center;
  gap: 0.25rem;

  &::after {
    content: url(/icons/question.svg);
    cursor: help;
    height: 0px;
    margin-block-end: 0.85rem;
  }
`;

function SearchHostForm({ onSubmit }) {
  const inputRef = React.useRef(null);
  const [hostMask, setHostMask] = React.useState("");
  const [errors, setErrors] = React.useState({});

  React.useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const handleSubmit = async (event) => {
    const response = await onSubmit(event);
    setErrors(response.errors);
  };

  const randomIpInput = () => {
    setErrors({});
    setHostMask(mask.masked(randomIPv4()));
  };

  return (
    <SearchWrapper>
      <Form errors={errors} onClearErrors={setErrors} onSubmit={handleSubmit}>
        <FieldRoot name="host">
          <Input
            ref={inputRef}
            type="text"
            inputMode="numeric"
            placeholder="Search for any IP address or domain"
            title="Search for any IP address or domain"
            value={hostMask}
            onValueChange={(value) => {
              setHostMask(mask.masked(value));
            }}
          />
          <ErrorMessage
            title={dedent`
              Valid inputs:
                • IPv4: typing only numbers and dots (optional) (e.g., 192.168.1.1)
                • IPv6: starting with “ ” (blank space), then enter numbers and colons (e.g.,  2001:db8::1)
                • Domain: no protocol or “www” (e.g., example.com)
            `}
          />
        </FieldRoot>
        <LongPressButton onLongPress={randomIpInput} />
      </Form>
    </SearchWrapper>
  );
}

export default SearchHostForm;
